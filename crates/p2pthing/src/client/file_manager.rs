use std::{collections::HashMap, convert::TryInto, env, ffi::OsStr, fs::{self, File, Metadata}, io::{self, BufReader, Read, Seek, SeekFrom}, path::{Path, PathBuf}, time::{Instant, SystemTime}};
use mio_misc::{channel::Sender};
use sha2::{Digest, Sha256};
use base64::encode_config;

use p2pthing_common::{encryption::NetworkedPublicKey, message_type::{FileChunk, FileDataChunk, FileId, InterthreadMessage, PreparedFile, msg_types::{FileChunks, RequestFileChunks}}, statistics::TransferStatistics, ui::UIConn};

mod chunk_writer;
use chunk_writer::ChunkWriter;

pub struct FileManager {
    pub transfer_statistics: HashMap<FileId, TransferStatistics>,
    open_files: HashMap<FileId, OpenFile>,
    /// Map the files to a vector where the index is the chunkindex and the value is whether it has been received.
    receiving_chunks: HashMap<FileId, Vec<ReceivableChunk>>,
    file_senders: HashMap<FileId, NetworkedPublicKey>,
    read_buffer: Vec<u8>,
    ui_s: Sender<InterthreadMessage>,
    /// Requests that haven't been sent to their respective peers
    new_requests: HashMap<NetworkedPublicKey, Vec<FileChunk>>
}

#[derive(Clone)]
struct ReceivableChunk {
    requested: bool,
    received: bool
}

impl ReceivableChunk {
    pub fn new() -> ReceivableChunk {
        ReceivableChunk {
            requested: false,
            received: false,
        }
    }
}

/// This struct holds an open file. It can either be a Reader or a Writer, but never both.
struct OpenFile {
    file: FileType,
    metadata: Metadata
}

enum FileType {
    Reader(BufReader<File>),
    Writer(ChunkWriter)
}


/// Max size of a single packet in bytes
const CHUNK_SIZE: usize = 1 * 1000;
/// This is where the file downloads will be placed
const DOWNLOADS_FOLDER: &str = "downloads";
/// Request this many chunks at the same time
const REQUESTED_CHUNK_COUNT: usize = 50;

impl FileManager {
    pub fn new(ui_s: Sender<InterthreadMessage>) -> FileManager {
        FileManager {
            transfer_statistics: HashMap::new(),
            open_files: HashMap::new(),
            receiving_chunks: HashMap::new(),
            file_senders: HashMap::new(),
            read_buffer: vec![0u8; CHUNK_SIZE],
            ui_s,
            new_requests: HashMap::new(),
        }
    }

    /// Prepare files for uploading
    pub fn send_files(&mut self, filenames: Vec<String>) -> io::Result<Vec<PreparedFile>> {
        let mut split_files = Vec::new();
        for filename in filenames {
            split_files.push(self.start_sending_file(filename)?);
        }
        return Ok(split_files);
    }

    pub fn start_sending_file(&mut self, filename: String) -> io::Result<PreparedFile> {
        let path = Path::new(&filename);
        let path = PathBuf::from(path);
        let filename = path.file_name().unwrap().to_str().unwrap();
        let extension = path.extension().unwrap_or(OsStr::new("")).to_string_lossy().to_string();

        let metadata = fs::metadata(path.clone())?;
        let total_length = metadata.len();

        let file_id = [&filename.as_bytes(), &total_length.to_be_bytes()[..]].concat();
        let file_id = Sha256::digest(&file_id);
        let file_id = encode_config(file_id, base64::URL_SAFE);
        
        // If the file is not already opened, then open it
        // TODO: What if the size of the file changes (someone else writes to it). Because then the hash would change.
        if !self.open_files.contains_key(&file_id) {
            self.open_file(&file_id, path.clone().as_path(), false, None)?;
        }

        return Ok(PreparedFile {
            file_id,
            file_name: filename.to_string(),
            file_extension: extension,
            total_length,
        })
    }

    pub fn start_receiving_file(&mut self, file: PreparedFile, sender: NetworkedPublicKey) -> io::Result<()> {
        let chunk_count: usize = file.total_length as usize / CHUNK_SIZE + 1;
        let original_name = PathBuf::from(file.file_name.clone());
        let download_path = env::current_dir().unwrap().join(PathBuf::from(DOWNLOADS_FOLDER)).join(file.file_id.clone()).with_extension(original_name.extension().unwrap());
        self.open_file(&file.file_id, &download_path, true, Some(file.total_length))?;

        // TODO: Enable receiving same file from multiple senders
        if !self.receiving_chunks.contains_key(&file.file_id) {
            let x = self.receiving_chunks.insert(file.file_id.clone(), vec![ReceivableChunk::new(); chunk_count.try_into().unwrap()]);
            assert!(x.is_none());
            let x = self.file_senders.insert(file.file_id.clone(), sender);
            assert!(x.is_none());
        }
        self.update_requested_chunks();
        Ok(())
    }

    /// Generate the file chunks that need to be requested
    fn update_requested_chunks(&mut self) {
        let mut total_requested = 0;
        for (file_id, chunks) in self.receiving_chunks.iter_mut() {
            // Skip chunks that are already downloaded
            for (index, chunk) in chunks.iter_mut().enumerate().skip_while(|(_, x)| x.requested && x.received) {
                if total_requested >= REQUESTED_CHUNK_COUNT { break; }
                if !chunk.requested {
                    let sender = self.file_senders.get(file_id).unwrap();
                    let peer_vec = match self.new_requests.get_mut(&sender) {
                        Some(v) => v,
                        None =>  {
                            self.new_requests.insert(sender.clone(), Vec::new()); 
                            self.new_requests.get_mut(&sender).unwrap()
                        },
                    };
                    peer_vec.push(FileChunk{file_id: file_id.clone(), index});
                    chunk.requested = true;
                }
                total_requested += 1;
            }
        }
    }

    pub fn get_requested_chunks(&mut self) -> Option<HashMap<NetworkedPublicKey, Vec<FileChunk>>> {
        if self.new_requests.len() > 0 {
            let requests = self.new_requests.clone();
            self.new_requests.clear();
            return Some(requests);
        }
        None
    }

    pub fn get_file_chunks(&mut self, request: RequestFileChunks) -> Result<Vec<FileDataChunk>, String> {
        let mut chunks: Vec<FileDataChunk> = Vec::new();
        for chunk in request.chunks {
            if let Some(f) = self.open_files.get_mut(&chunk.file_id) {
                let start_file_index = chunk.index * CHUNK_SIZE;
                let remaining_total_file = f.metadata.len() as usize - start_file_index;
                let end_file_index = start_file_index + remaining_total_file.min(CHUNK_SIZE);
                let read_bytes = end_file_index - start_file_index;

                if let FileType::Reader(reader) = &mut f.file {
                    if let Err(e) = reader.seek(SeekFrom::Start(start_file_index as u64)) {
                        return Err(format!("Failed seeking in a file: ({};{}) {}", chunk.file_id.clone(), start_file_index.clone(), e));
                    }
                    
                    if let Err(e) = reader.read_exact(&mut self.read_buffer[0..read_bytes]) {
                        return Err(format!("Failed reading from a file: ({}[{}]) {}", chunk.file_id.clone(), chunk.index.clone(), e));
                    }
    
                    let stats = self.transfer_statistics.get_mut(&chunk.file_id).unwrap();
                    stats.bytes_read += read_bytes;
    
                    chunks.push(FileDataChunk {
                        file_id: chunk.file_id.clone(),
                        index: chunk.index,
                        data: self.read_buffer[0..read_bytes].to_vec(),
                    });
                }
                else {
                    self.ui_s.log_error(&format!("Tried to read from a file, but couldn't find reader: ({})", chunk.file_id.clone()));
                }
            }
            else {
                return Err(format!("Tried reading from a non existing file: ({})", chunk.file_id.clone()));
            }
        }
        Ok(chunks)
    }

    pub fn store_file_chunks(&mut self, msg: FileChunks) -> Result<(), String> {
        let mut files_changed = Vec::new();
        for chunk in msg.chunks {
            if let Some(f) = self.open_files.get_mut(&chunk.file_id) {
                let chunk_list = self.receiving_chunks.get_mut(&chunk.file_id).unwrap();
                if !chunk_list[chunk.index].received {
                    let index_start = chunk.index * CHUNK_SIZE;
                    
                    if let FileType::Writer(writer) = &mut f.file {
                        if let Err(e) = writer.write_chunk(chunk.index, &chunk.data[..]) {
                            return Err(format!("Failed writing to a file: ({}[{}]) {}", chunk.file_id.clone(), index_start.clone(), e));
                        }
                        
                        let chunk_list = self.receiving_chunks.get_mut(&chunk.file_id).unwrap();
                        chunk_list[chunk.index].received = true;
    
                        let stats = self.transfer_statistics.get_mut(&chunk.file_id).unwrap();
                        stats.bytes_written += chunk.data.len();
    
                        self.update_requested_chunks();
    
                        if !files_changed.contains(&chunk.file_id) {
                            files_changed.push(chunk.file_id.clone());
                        }
                    }
                    else {
                        self.ui_s.log_error(&format!("Tried to write to a file, but couldn't find writer: ({})", chunk.file_id.clone()));
                    }
                }
            }
            else {
                return Err(format!("Tried writing to a non existing file: ({})", chunk.file_id.clone()));
            }
        }
        self.check_files_done(files_changed);
        Ok(())
    }

    fn check_files_done(&mut self, files: Vec<FileId>) {
        for file in files {
            if self.is_file_done(&file) {
                let open_file = self.open_files.remove(&file).unwrap();
                self.receiving_chunks.remove(&file).unwrap();
                self.file_senders.remove(&file).unwrap();

                // TODO: Properly notify UI
                let stats = self.transfer_statistics.remove(&file).unwrap();
                
                if let Ok(elapsed) = stats.started.elapsed() {
                    let secs = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;
                    let mbs = open_file.metadata.len() as f64 / 1000f64 / 1000f64  / secs ;
                    self.ui_s.log_info(&format!("Finished file ({}) in {}ms achieving {:.} MB/s", &file[0..10], elapsed.as_millis(), mbs));
                }

                drop(open_file);
            }
        }
    }

    fn is_file_done(&self, file_id: &FileId) -> bool {
        self.receiving_chunks.get(file_id).unwrap().iter().all(|x| x.received)
    }

    fn open_file(&mut self, file_id: &FileId, path: &Path, create: bool, set_file_length: Option<u64>) -> io::Result<()> {
        if !self.open_files.contains_key(file_id) {
            let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .open(path.clone())?;

            if let Some(len) = set_file_length {
                file.set_len(len)?;
            }

            let metadata = file.metadata()?;
            let file = match create {
                true => {
                    self.ui_s.log_info(&format!("Opened a file for writing: {}", path.clone().to_str().unwrap()));
                    FileType::Writer(ChunkWriter::new(file, CHUNK_SIZE, REQUESTED_CHUNK_COUNT))
                },
                false => {
                    self.ui_s.log_info(&format!("Opened a file for reading: {}", path.clone().to_str().unwrap()));
                    FileType::Reader(BufReader::new(file))
                },
            };

            self.open_files.insert(file_id.clone(), OpenFile {
                file,
                metadata
            });
            self.transfer_statistics.insert(file_id.clone(), TransferStatistics::new());
        }
        Ok(())
    }
}