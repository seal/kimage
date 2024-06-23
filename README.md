Cheap and cheerful image server + uploader 



## Installation

```
cargo install kimage
```

## Config
* must be in ~/.config/kimage.toml on server and local *
```toml
server_url="https://img.domain.com"
port=8001
api_key="inserthere"
storage_path="/hard-path/to/images"
```

## Usage ( server ) 
Run kimage-serve on the server

Have appropriate https, domain etc set up

## Usage ( local ) 

```
kimage IMAGE.png
```

URL will be copied to clipboard 
