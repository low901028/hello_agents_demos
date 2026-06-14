use std::collections::HashMap;
use chrono::Utc;
use serde_json::Value;
use crate::core::types::message::{Message, MessageContent, MessageRole};

#[derive(Clone)]
pub struct HistoryManager {
    history: Vec<Message>,
    pub min_retain_rounds: usize,
    pub compression_threshold: f64,
}

impl HistoryManager {
    pub fn new(min_retain_rounds: usize, compression_threshold: f64) -> Self { Self { history: vec![], min_retain_rounds, compression_threshold } }
    pub fn append(&mut self, msg: Message) { self.history.push(msg); }
    pub fn get_history(&self) -> Vec<Message> { self.history.clone() }
    pub fn clear(&mut self) { self.history.clear(); }
    pub fn estimate_rounds(&self) -> usize { /* 同前 */ 0 }
    pub fn find_round_boundaries(&self) -> Vec<usize> { /* 同前 */ vec![] }
    pub fn compress(&mut self, summary: &str) { /* 同前 */ }
    pub fn to_dict(&self) -> HashMap<String, Value> { /* 同前 */ HashMap::new() }
    pub fn load_from_dict(&mut self, data: &HashMap<String, Value>) { /* 同前 */ }
}