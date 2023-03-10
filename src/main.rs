use rust_socketio::{ClientBuilder, Payload, RawClient};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::{io, io::prelude::*};

fn give_prompt() {
    io::stderr().write_all(("empire search > ").as_bytes()).unwrap();
    io::stderr().flush().unwrap();
}

fn get_input() -> String {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct SearchResponse {
    films: String,
    name: String,
    page: u64,
    // camelCase to help with deserialization
    resultCount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct SearchError {
    error: String,
    page: i64,
    // camelCase to help with deserialization
    resultCount: i64,
}

#[derive(Debug, Serialize, Deserialize)]
enum RawSearchResponse {
    Success(SearchResponse),
    Error(SearchError),
}

// Parse a string of JSON into a RawSearchResponse
fn parse_raw_search_response(json: &str) -> Result<RawSearchResponse, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    if value["error"].is_string() {
        let error: SearchError = serde_json::from_value(value)?;
        Ok(RawSearchResponse::Error(error))
    } else {
        let response: SearchResponse = serde_json::from_value(value)?;
        Ok(RawSearchResponse::Success(response))
    }
}

fn response_handler(is_done: Sender<bool>, payload: Payload, _socket: RawClient) {
    match payload {
        Payload::String(str_payload) => {
            // sample response: {"films":"A New Hope, The Empire Strikes Back, Return of the Jedi, Revenge of the Sith","name":"Darth Vader","page":1,"resultCount":3}
            let response: Result<RawSearchResponse, serde_json::Error> =
                parse_raw_search_response(&str_payload);
            match response {
                Ok(RawSearchResponse::Success(response)) => {
                    println!(
                        "({}/{}) {} - [{}]",
                        response.page, response.resultCount, response.name, response.films
                    );
                    if response.page >= response.resultCount {
                        _ = is_done.send(true);
                    }
                }
                Ok(RawSearchResponse::Error(response)) => {
                    println!("{}", response.error);
                    _ = is_done.send(true);
                }
                Err(err) => {
                    println!("Error parsing response: {:#?}", err);
                    _ = is_done.send(true);
                }
            }
        }
        Payload::Binary(bin_data) => {
            println!("Unexpectedly got bytes: {:#?}", bin_data);
            _ = is_done.send(true);
        }
    }
}

fn make_response_handler(is_done: Sender<bool>) -> impl Fn(Payload, RawClient) {
    move |payload: Payload, socket: RawClient| {
        response_handler(is_done.clone(), payload, socket);
    }
}

fn main() {
    env_logger::init();
    println!("Press Ctrl+C to exit");
    loop {
        let (is_done, rx) = mpsc::channel();
        let handler = make_response_handler(is_done);
        give_prompt();
        let socket = ClientBuilder::new("http://localhost:3000")
            .on("search", handler)
            .on("error", |err, _| eprintln!("Error: {:#?}", err))
            .connect()
            .expect("Connection failed");
        let input = get_input();
        let _result = socket
            .emit("search", json!({ "query": input }))
            .expect("Failed to emit");
        // Block on a channel waiting for the handler to signal completion.
        let _is_done = rx.recv();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_raw_search_response() {
        let json = r#"{"films":"A New Hope, The Empire Strikes Back, Return of the Jedi, Revenge of the Sith","name":"Darth Vader","page":1,"resultCount":3}"#;
        let response: Result<RawSearchResponse, serde_json::Error> =
            parse_raw_search_response(&json);
        match response {
            Ok(RawSearchResponse::Success(response)) => {
                assert_eq!(response.page, 1);
                assert_eq!(response.resultCount, 3);
                assert_eq!(response.name, "Darth Vader");
                assert_eq!(
                    response.films,
                    "A New Hope, The Empire Strikes Back, Return of the Jedi, Revenge of the Sith"
                );
            }
            _ => panic!("Unexpected response: {:#?}", response),
        }
    }

    #[test]
    fn test_parse_raw_search_response_error() {
        let json = r#"{"error":"No results found","page":-1,"resultCount":-1}"#;
        let response: Result<RawSearchResponse, serde_json::Error> =
            parse_raw_search_response(&json);
        match response {
            Ok(RawSearchResponse::Error(response)) => {
                assert_eq!(response.page, -1);
                assert_eq!(response.resultCount, -1);
                assert_eq!(response.error, "No results found");
            }
            _ => panic!("Unexpected response: {:#?}", response),
        }
    }

    #[test]
    fn test_parse_raw_search_response_invalid() {
        let json = r#"{"error":"No results found","page":-1,"resultCount":-1"#;
        let response: Result<RawSearchResponse, serde_json::Error> =
            parse_raw_search_response(&json);
        match response {
            Err(err) => {
                assert!(err.to_string().contains("EOF while parsing"));
            }
            _ => panic!("Unexpected response: {:#?}", response),
        }
    }
}
