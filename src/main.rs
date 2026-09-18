use std::collections::HashMap;
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
        "Set" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            let value = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Set { key, value })
        }
        "Get" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Get { key })
        }
        "Delete" => {
            let key = a.next().ok_or(ParseError::InvalidArguments)?.to_string();
            Ok(Command::Delete { key })
        }
        _ => Err(ParseError::UnknownCommand),
    }
}
}   
fn main(){
    let mut db = Database::new();
    let commands =["Set name shini","Get name","Delete name","Get name"];
    for input in commands{
        match Database::parse_command(input){
            Ok(command)=>{let response = db.excute(command);
            println!("{:?}",response);
        }
            Err(error)=>{
                println!("{:?}",error)


            }


        }


    }
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