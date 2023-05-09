use std::borrow::Borrow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::net::SocketAddr;
use std::string::ParseError;
use std::time::SystemTime;
use url::Url;
use serde::{Deserialize, Serialize};
use serde_json::to_string;
use crate::http_server::BUFFER_SIZE;
use crate::error::{InvalidMethodError, RequestParseError, ServerError};
use crate::error::DefaultError::RequestParse;
use crate::method::HttpMethod;

#[derive(Debug)]
pub struct Request {
    socket_addr: SocketAddr,
    method: HttpMethod,
    route: String,
    protocol: String,
    version: f32,
    host: String,
    headers: HashMap<String, String>,
    query: HashMap<String, String>,
    body: Vec<u8>,
    url: Url
}

impl Request {
    pub fn new(socket_addr: SocketAddr, method: HttpMethod, url: Url, version: f32, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        let query = url.query_pairs()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();

        Self {
            socket_addr,
            method,
            route: url.path().to_string(),
            protocol: url.scheme().to_string(),
            version,
            host: url.host_str().unwrap().to_string(),
            headers,
            query,
            url,
            body
        }
    }

    pub fn from_bytes(socket_addr: SocketAddr, bytes: &[u8]) -> Result<Self, RequestParseError> {
        let head_len = bytes.windows(4).position(|window| matches!(window, b"\r\n\r\n")).unwrap_or(bytes.len());
        let data = std::str::from_utf8(&bytes[..head_len]).map_err(|_| RequestParseError::MalformedRequest)?;
        let mut lines = data.split("\r\n");
        let mut first = lines.next().ok_or(RequestParseError::MalformedRequest)?.split(' ');
        let method = HttpMethod::try_from(first.next().unwrap_or(""))?;
        let route = first.next().ok_or(RequestParseError::Route)?;
        let mut v = first.next().ok_or(RequestParseError::Protocol)?.split("/");
        let protocol = v.next().ok_or(RequestParseError::Protocol)?.to_ascii_lowercase();
        let version: f32 = v.next().ok_or(RequestParseError::Protocol)?.parse().map_err(|_| RequestParseError::Protocol)?;
        let mut headers = HashMap::new();

        while let Some(line) = lines.next() {
            let mut l = line.split(": ");
            let header = l.next().ok_or(RequestParseError::MalformedRequest)?.to_ascii_lowercase();
            headers.insert(header.to_string(), l.next().ok_or(RequestParseError::Header(header))?.to_string());
        }

        let host = headers.get("host").ok_or(RequestParseError::Host)?;
        let body = vec![];

        let body = if head_len == bytes.len() {
            body
        } else {
            match headers.get("content-length") {
                Some(len) => {
                    let len = len.parse::<usize>().map_err(|_| RequestParseError::Header("content-length".to_string()))?;
                    bytes[head_len + 4..head_len + 4 + len].to_vec()
                },
                None => body
            }
        };

        let url = Url::parse(format!("{protocol}://{host}{route}").as_str()).map_err(|_| RequestParseError::Route)?;
        Ok(Self::new(socket_addr, method, url, version, headers, body))
    }

    pub fn socket_addr(&self) -> SocketAddr {
        self.socket_addr
    }

    pub fn method(&self) -> HttpMethod {
        self.method
    }

    pub fn route(&self) -> &str {
        &self.route
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn version(&self) -> f32 {
        self.version
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn header(&self, header: &str) -> Option<&str> {
        self.headers.get(header).map(|value| value.as_str())
    }

    pub fn text(&self) -> Result<&str, RequestParseError> {
        std::str::from_utf8(&self.body).map_err(|_| RequestParseError::Body)
    }

    pub fn json<'a, T: Deserialize<'a>>(&'a self) -> Result<T, RequestParseError> {
        println!("{:?}", self.body);
        serde_json::from_slice(&self.body).map_err(|_| RequestParseError::Body)
    }

    pub fn raw(&self) -> &[u8] {
        &self.body
    }
}

#[derive(Debug)]
pub struct Response {
    protocol: String,
    version: f32,
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>
}

impl Response {
    pub fn new(status: u16) -> Self {
        Self {
            protocol: String::new(),
            version: 0.0,
            headers: HashMap::new(),
            body: Vec::new(),
            status
        }
    }

    pub fn text(text: impl Display, status: u16) -> Self {
        let mut response = Response::new(status);
        response.set_body(text.to_string().as_bytes(), "text/html").unwrap();
        response
    }

    pub fn file(filename: &str, status: u16) -> io::Result<Self> {
        let mut response = Self::new(status);
        let mut file = BufReader::new(File::open(filename)?);
        response.set_body(file, &Self::file_content_type(filename))?;
        Ok(response)
    }

    pub fn json(json: impl Serialize, status: u16) -> serde_json::Result<Self> {
        let mut response = Response::new(status);
        let serialized = serde_json::to_string(&json)?;
        response.set_body(serialized.as_bytes(), "application/json").unwrap();
        Ok(response)
    }

    pub fn fill_from(&mut self, request: &Request) {
        self.version = request.version;
        self.protocol = request.protocol.to_string();
    }

    pub fn set_body(&mut self, mut body: impl Read, content_type: &str) -> io::Result<()> {
        let start = SystemTime::now();
        let mut buffer = [0_u8; BUFFER_SIZE];

        while let size = body.read(&mut buffer)? {
            self.body.extend_from_slice(&buffer[..size]);

            if size < BUFFER_SIZE {
                break;
            }
        }

        println!("Reading response data took {} ms", start.elapsed().unwrap().as_millis());
        self.header("Content-Length", &self.body.len().to_string());
        self.header("Content-Type", content_type);
        Ok(())
    }

    pub fn header(&mut self, header: &str, value: &str) {
        self.headers.insert(header.to_string(), value.to_string());
    }

    pub fn to_bytes(mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice((self.protocol.to_ascii_uppercase() + "/").as_bytes());
        bytes.extend_from_slice((self.version.to_string() + " ").as_bytes());
        bytes.extend_from_slice((self.status.to_string() + "\r\n").as_bytes());

        for (header, value) in self.headers {
            bytes.extend_from_slice((header + ": ").as_bytes());
            bytes.extend_from_slice((value + "\r\n").as_bytes());
        }

        bytes.extend_from_slice("\r\n".as_bytes());

        if self.body.len() > 0 {
            bytes.extend_from_slice(&self.body);
            bytes.extend_from_slice("\r\n\r\n".as_bytes());
        }

        bytes
    }

    fn file_content_type(filename: &str) -> String {
        let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();

        let content_type = match extension.as_str() {
            "html" | "htm" => "text/html; charset=utf-8",
            "txt" => "text/plain; charset=utf-8",
            "js" => "application/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "xml" => "text/xml; charset=utf-8",
            "csv" => "text/csv; charset=utf-8",
            "json" => "application/json; charset=utf-8",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "oga" => "audio/ogg",
            "ogv" => "video/ogg",
            "ogg" | "ogx" => "application/ogg",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            "mpeg" => "video/mpeg",
            "mp4" => "video/mp4",
            "wav" => "audio/wav",
            "weba" => "audio/webm",
            "webp" => "image/webp",
            "webm" => "video/webm",
            "aac" => "audio/aac",
            "abw" => "application/x-abiword",
            "arc" => "application/x-freearc",
            "7z" => "application/x-7z-compressed",
            "3g2" => "video/3gpp2",
            "3gp" => "video/3gpp",
            "xul" => "application/vnd.mozilla.xul+xml",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xls" => "application/vnd.ms-excel",
            "xhtml" => "application/xhtml+xml",
            "woff2" => "font/woff2",
            "woff" => "font/woff",
            "tar" => "application/x-tar",
            "tif" | "tiff" => "image/tiff",
            "ts" => "video/mp2t",
            "ttf" => "font/ttf",
            "php" => "application/x-httpd-php",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "rar" => "application/vnd.rar",
            "rtf" => "application/rtf",
            "sh" => "application/x-sh",
            "svg" => "image/svg+xml",
            "vsd" => "application/vnd.visio",
            "mpkg" => "application/vnd.apple.installer+xml",
            "odp" => "application/vnd.oasis.opendocument.presentation",
            "ods" => "application/vnd.oasis.opendocument.spreadsheet",
            "odt" => "application/vnd.oasis.opendocument.text",
            "opus" => "audio/opus",
            "otf" => "font/otf",
            "jsonld" => "application/ld+json",
            "mid" | "midi" => "audio/midi",
            "mjs" => "text/javascript",
            "mp3" => "audio/mpeg",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "eot" => "application/vnd.ms-fontobject",
            "epub" => "application/epub+zip",
            "gz" => "application/gzip",
            "bmp" => "image/bmp",
            "bz" => "application/x-bzip",
            "bz2" => "application/x-bzip2",
            "cda" => "application/x-cdf",
            "csh" => "application/x-csh",
            "avif" => "image/avif",
            "avi" => "video/x-msvideo",
            "azw" => "application/vnd.amazon.ebook",
            _ => "application/octet-stream"
        };

        String::from(content_type)
    }
}
