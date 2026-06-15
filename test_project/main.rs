// This is a test file for mrgfiles
fn main() {
    println!("Hello, world!");
}

/*
  A multiline comment
  with some text
*/
pub struct MyStruct {
    pub value: i32,
}

impl MyStruct {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn get_value(&self) -> i32 {
        self.value
    }
}
