use std::collections::HashMap;
use tokio::net::{TcpStream, TcpListener};
use std::sync::Arc;
use tokio::sync::Mutex;
type SharedDatabase = Arc<Mutex<Database>>;
use tokio::io::{AsyncBufReadExt,AsyncWriteExt,BufReader};
#[derive(Debug)]
enum ParseError {
    EmptyCommand,
    UnknownCommand,
    InvalidArguments,
}
#[derive(Debug)]
enum Response {
    Ok,
    Value(Option<String>),
    Deleted(bool),
}
#[derive(Debug)]
enum Command{
    SET{
        key:String,
        value:String
    },
    GET{
        key:String,
    },
    DELETE{
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
    fn SET(&mut self,key:String,value:String){
        self.kv.insert(key,value);
    }
    fn GET(&self,key:&str)->Option<&str>{
      self.kv.get(key).map(|v|v.as_str())
    }
    fn DELETE(&mut self,key:&str)->bool{
        self.kv.remove(key).is_some()
    }
    fn excute(&mut self,command:Command)->Response{
        match command{
            Command::SET{key,value}=>{self.SET(key,value);Response::Ok},
            Command::GET{key}=>{    Response::Value(self.GET(&key).map(|s| s.to_string()))},
            Command::DELETE{key}=>{Response::Deleted(self.DELETE(&key))},

        }


    }
    fn parse_command(input: &str) -> Result<Command, ParseError> {
    let mut a =input.split_whitespace();
    let cmd = a.next().ok_or(ParseError::EmptyCommand)?;
    match cmd {
        "Set" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            let value = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::SET { key, value })
        }
        "Get" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::GET { key })
        }
        "Delete" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::DELETE { key })
        }
        _ => Err(ParseError::UnknownCommand),
    }
}
}   
#[tokio::main]
async fn main(){
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let db =Arc::new(Mutex::new(Database::new()));
    loop{
        let (socket,_) = listener.accept().await.unwrap();
        println!("successful");
        let db =db.clone();
        tokio::spawn(async move{process(socket,db).await;});


    }
   
}
async fn process(socket:TcpStream,db:SharedDatabase)->Result<(),Box<dyn std::error::Error+Send + Sync>>{
    
    
    let mut line = String::new();
    let (read_half,mut writer)=socket.into_split();
    let mut reader = BufReader::new(read_half);
    loop{
        line.clear();
        let size = reader.read_line(&mut line).await?;
        if size ==0 {break;}
        let input = line.trim_end_matches(&['\r', '\n'][..]);
        let response = match Database::parse_command(input){
            Ok(command)=>{println!("successful{:?}",command);let mut db = db.lock().await;db.excute(command)},
             Err(error) => {
                 let output = format!("ERR {:?}\n", error);
                writer.write_all(output.as_bytes()).await?;
                 continue;
            }

            
            

        };
let output = match response {
    Response::Ok => "OK\n".to_string(),
    Response::Value(Some(value)) => format!("VALUE {}\n", value),
    Response::Value(None) => "NOT_FOUND\n".to_string(),
    Response::Deleted(true) => "DELETED\n".to_string(),
    Response::Deleted(false) => "NOT_FOUND\n".to_string(),
};
writer.write_all(output.as_bytes()).await?;
       
    }   
     
   Ok(())


}




#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test_set_and_get(){
        let mut db =Database::new();
        let command =Database::parse_command("Set name shini").unwrap();
        let response =db.excute(command);
        assert!(matches!(response,Response::Ok));
        let command = Database::parse_command("Get name").unwrap();
        let response =db.excute(command);
        assert!(matches!(response,Response::Value(Some(value))if value =="shini"));
        let command = Database::parse_command("Delete name").unwrap();
        let response = db.excute(command);
        assert!(matches!(response,Response::Deleted(true)));
        let command = Database::parse_command("Get name").unwrap();
        let response =db.excute(command);
        assert!(matches!(response,Response::Value(None)));

    }




}