use std::collections::HashMap;
#[derive(Debug)]
enum ParseError {
    EmptyCommand,
    UnknownCommand,
    InvalidArguments,
}
enum Response {
    Ok,
    Value(Option<String>),
    Deleted(bool),
}
enum Command{
    Set{
        key:String,
        value:String
    },
    Get{
        key:String,
    },
    Delete{
        key:String,
    }



}
struct Database{
    kv:HashMap<String,String>,
}
impl Database{
    fn new()->Self{
        Database{kv:HashMap::new()}
    }
    fn set(&mut self,key:String,value:String){
        self.kv.insert(key,value);
    }
    fn get(&self,key:&str)->Option<&str>{
      self.kv.get(key).map(|v|v.as_str())
    }
    fn delete(&mut self,key:&str)->bool{
        self.kv.remove(key).is_some()
    }
    fn excute(&mut self,command:Command)->Response{
        match command{
            Command::Set{key,value}=>{self.set(key,value);Response::Ok},
            Command::Get{key}=>{    Response::Value(self.get(&key).map(|s| s.to_string()))},
            Command::Delete{key}=>{Response::Deleted(self.delete(&key))},

        }


    }
    fn parse_command(input: &str) -> Result<Command, ParseError> {
    let mut a =input.split_whitespace();
    let cmd = a.next().ok_or(ParseError::EmptyCommand)?;
    match cmd {
        "SET" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            let value = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Set { key, value })
        }
        "GET" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Get { key })
        }
        "DELETE" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Delete { key })
        }
        other => Err(ParseError::UnknownCommand),
    }
}
}   
fn main(){
    let mut db = Database::new();
    db.set("name".to_string(),"shinni".to_string());
    println!("{:?}",db.get("name"));
    println!("{:?}",db.delete("name"));
    println!("{:?}",db.get("name"));
}