use std::cmp::{max, min};

pub struct ChatInput{
    pub msg: Vec<char>,
    cursor_position: usize
}

// TODO: Look at how rust handles Strings (bytes vs characters)
impl ChatInput {
    pub fn new() -> ChatInput {
        ChatInput {
            msg: vec![],
            cursor_position: 0,
        }
    }

    pub fn get_string(&self) -> String {
        self.msg.iter().collect()
    }

    pub fn clear(&mut self) {
        self.msg.clear();
        self.cursor_position = 0;
    }

    pub fn push_char(&mut self, c: char) {
        if self.msg.len() == 0 || (self.cursor_position == self.msg.len() - 1) {
            self.msg.push(c);
        }
        else {
            self.msg.insert(self.cursor_position, c);
        }
        self.cursor_position += 1;
    }

    pub fn advance_cursor(&mut self) {
        if self.msg.len() > 0 && self.cursor_position < self.msg.len() - 1 {
            self.cursor_position += 1;
        }
    }

    pub fn deadvance_cursor(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn get_cursor_pos(&self) -> usize {
        self.cursor_position
    }

    pub fn backspace(&mut self) {
        if self.msg.len() == 0 {return;}
        if self.cursor_position - 1 == self.msg.len() - 1 {
            self.msg.pop();
            self.cursor_position -= 1;
        }
        else if self.cursor_position != 0{
            self.msg.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.msg.len() == 0 {return;}
        if self.cursor_position <= self.msg.len() - 1 {
            self.msg.remove(self.cursor_position);
        }
    }
}