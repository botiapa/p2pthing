# P2PThing

## A cross platform peer-to-peer chat and voip application written in rust

![Screenshot](/tui_screenshot.png?raw=true)

## Usage

-   Download a built binary
-   Or build it yourself: - Run the rendezvous server: `cargo run --release s` - Run the client(s): `cargo run --release --features tui,audio c`

In case you are getting build errors when building with the audio feature enabled, make sure c++ tools are installed and if on windows add cmake to your PATH environment variable.

By default the client will try to connect to `127.0.0.1:42069`. However if you want to specify the IP, then run the client like this: `cargo run --release --features client,audio c 192.168.10.30:42069`, where `192.168.10.30` is the ip and `42069` is the port obviously.

## Implemented Features

-   Multi peer chat
-   UDP Punchthrough
-   Encryption on all communications
    -   Asymmetric RSA encryption while in handshaking phase
    -   Symmetric AES-256 encryption once connected
-   Audio support
    -   Opus encoded
    -   Variable bitrate (Down to 2 kbit/s)
    -   Input resampling
    -   Output resampling
    -   Togglable noise suppression
-   TUI (Terminal User Interface)
-   Cross platfrom support
    -   Windows - Full support
    -   Linux - I'll do my best to support linux
    -   Mac - No idea, don't have one, altough probably could work

## Features that could be implemented

-   Screen sharing
    -   Probably with ffmpeg
        -   DXGI on windows
    -   Capturing and streaming application audio
        -   Either with a loopback device
        -   Or by directly hooking into an application, like how discord does it
-   File sharing

## Security Notice

I DON'T HAVE ANY SECURITY BACKGROUND AND THIS APPLICATION HASN'T BEEN AUDITED, SO DO NOT TRUST THE ENCRYPTION. I DON'T RECOMMEND USING THIS APPLICATION WITH ANY SENSITIVE DATA.
