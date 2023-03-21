/**
 StreamParser
  
 Input Stream: 
data: {"Hello": "World"}

data: {"abc": ["def", "ad\\n2a"]}

The stream may be input by chunks, for example:
1: dat
2: a: {"Hello": "World"}
3: 
4: data: {"abc": ["def", "ad\\n2a"]}

Want to output a stream of JSON objects, represented as string.
1: {"Hello": "World"}
2: {"abc": ["def", "ad\\n2a"]}
 */

 use std::str;
 use async_stream::try_stream;
use worker::{ByteStream, console_log};
use futures_util::TryStream;
use futures_util::{StreamExt, TryStreamExt};

 pub struct StreamParser {
     buffer: String,
 }
 
 impl StreamParser {
     pub fn new() -> Self {
         Self {
             buffer: String::new(),
         }
     }
 
     pub fn add_chunk(&mut self, chunk: &[u8]) {
         if let Ok(chunk_str) = str::from_utf8(chunk) {
             self.buffer.push_str(chunk_str);
         }
     }
 
     pub fn next(&mut self) -> Option<String> {
         if let Some(start) = self.buffer.find("data:") {
             let json_start = start + "data:".len();
             let json_end = self.buffer[json_start..].find('\n').map(|i| json_start + i);
             if let Some(end) = json_end {
                 let json_str = self.buffer[json_start..end].trim().to_string();
                 self.buffer.drain(..end);
                 return Some(json_str);
             }
         }
         None
     }
 
     pub fn is_empty(&self) -> bool {
         self.buffer.is_empty()
     }

     pub fn parse_byte_stream(stream: ByteStream) -> impl TryStream<Item = worker::Result<String>> + Unpin {
        Box::pin(try_stream! {
            let mut parser = StreamParser::new();
            let mut stream = stream.into_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                parser.add_chunk(&chunk);
                while let Some(json) = parser.next() {
                    yield json;
                }
            }
        })
    }
 }