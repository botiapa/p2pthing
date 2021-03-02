# P2PThing
## A cross platform peer-to-peer chat and voip application written in rust

![Screenshot](/tui_screenshot.png?raw=true)

## Implemented Features
- Multi peer chat
- UDP Punchthrough
- Encryption on all communications
    - Asymmetric RSA encryption while in handshaking phase
    - Symmetric AES-256 encryption once connected
- Audio support
    - Opus encoded
    - Variable bitrate (Down to 2 kbit/s)
    - Input resampling
    - Output resampling
    - Togglable noise suppression
- TUI (Terminal User Interface)
- Cross platfrom support
    - Windows - Full support
    - Linux - I'll do my best to support linux
    - Mac - No idea, don't have one, altough probably could work

## Planned Features
- Screen sharing
    - Probably with ffmpeg
        - DXGI on windows
    - Capturing and streaming application audio
        - Either with a loopback device
        - Or by directly hooking into an application, like how discord does it
- File sharing

## Security Notice

I DON'T HAVE ANY SECURITY BACKGROUND AND THIS APPLICATION HASN'T BEEN AUDITED, SO DO NOT TRUST THE ENCRYPTION. I DON'T RECOMMEND USING THIS APPLICATION WITH ANY SENSITIVE DATA.

