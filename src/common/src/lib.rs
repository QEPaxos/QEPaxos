pub mod config;

use rand;

pub const CONFLICT_KEY: &str = "50";

#[derive(Debug)]
pub struct Instance {
    pub replica: i32,
    pub instance: usize,
    pub ballot: i32,
}

// remove or add http:// prefix
pub fn convert_ip_addr(ip: String, add_http: bool) -> String {
    if add_http {
        let prefix = String::from("http://");
        prefix + ip.clone().as_str()
    } else {
        let len = ip.len();
        if len <= 8 {
            return String::from("");
        }
        let result = &ip[7..len];
        result.to_string()
    }
}

pub fn convert_self_ip(port: i32) -> String {
    let prefix = String::from("localhost:");
    prefix + port.to_string().as_str()
}

pub struct KeyGenerator {
    conflict_rate: u32,
    pub value: String,
}

impl KeyGenerator {
    pub fn new(conflict_rate: u32, value_size: usize) -> Self {
        Self {
            conflict_rate,
            value: String::from("0").repeat(value_size),
        }
    }
    pub fn gen_request(&self, len: usize, replica: usize) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();

        for i in 0..len {
            let r: u32 = rand::random::<u32>() % 100;
            if r < self.conflict_rate {
                result.push(CONFLICT_KEY.to_string());
            } else {
                result.push(replica.to_string());
            }
        }
        result
    }
}
