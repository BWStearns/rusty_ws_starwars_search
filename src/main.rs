
use rust_socketio::{ClientBuilder, Payload, RawClient};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io, io::prelude::*};

fn give_prompt() {
    io::stderr().write_all(("empire search > ").as_bytes()).unwrap();
    io::stderr().flush().unwrap();
}

fn get_input() -> String {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let res = input.trim().to_string();
    res
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResponse {
    films: String,
    name: String,
    page: u64,
    // camelCase to help with deserialization
    resultCount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
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

// This is to allow us to block while waiting for a response from the server
static IS_HANDLING_RESPONSES: AtomicBool = AtomicBool::new(false);

fn response_handler(payload: Payload, socket: RawClient) {
    match payload {
        Payload::String(str_payload) => {
            // sample response: {"films":"A New Hope, The Empire Strikes Back, Return of the Jedi, Revenge of the Sith","name":"Darth Vader","page":1,"resultCount":3}
            let response: Result<RawSearchResponse, serde_json::Error> = parse_raw_search_response(&str_payload);
            match response {
                Ok(RawSearchResponse::Success(response)) => {
                    println!("({}/{}) {} - [{}]", response.page, response.resultCount, response.name, response.films);
                    if response.page >= response.resultCount {
                        IS_HANDLING_RESPONSES.swap(false, Ordering::Relaxed);
                        _ = socket.disconnect();
                    }
                }
                Ok(RawSearchResponse::Error(response)) => {
                    println!("{}", response.error);
                    IS_HANDLING_RESPONSES.swap(false, Ordering::Relaxed);
                    _ = socket.disconnect();
                }
                Err(err) => {
                    println!("Error parsing response: {:#?}", err);
                    IS_HANDLING_RESPONSES.swap(false, Ordering::Relaxed);
                    _ = socket.disconnect();
                }
            }
        }
        Payload::Binary(bin_data) => println!("Unexpectedly got bytes: {:#?}", bin_data),
    }
}

fn main() {
    env_logger::init();
    println!("Press Ctrl+C to exit");
    loop {
        IS_HANDLING_RESPONSES.swap(true, Ordering::Relaxed);
        give_prompt();
        let socket = ClientBuilder::new("http://localhost:3000")
            .on("search", response_handler)
            .on("error", |err, _| eprintln!("Error: {:#?}", err))
            .connect()
            .expect("Connection failed");
        let input = get_input();
        let _result = socket.emit("search", json!({ "query": input })).expect("Failed to emit");
        while IS_HANDLING_RESPONSES.load(Ordering::Relaxed) {
            // wait for the server to respond
        }
    }
}
