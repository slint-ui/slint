use servo::{Servo,ServoBuilder};
use slint::Image; 
use std::path::Path; 
pub struct Browser { 
    current_url: String, 
} 
impl Browser { 
    pub fn new() -> Self { 
        Browser { current_url: String::new() } 
    } 
 
    pub fn load_page(&mut self, url: &str) { 
         
        self.current_url = url.to_string(); 
        println!("Servo browser is loading: {}", self.current_url); 
 
    } 
    pub fn render_view_as_image(&self) -> slint::Image { 
         
        slint::Image::load_from_path(Path::new("placeholder.png")).unwrap() 
    } 
    pub fn current_page(&self) -> &str { 
        &self.current_url 
    } 
} 
 
pub fn servo_start() -> Browser { 
    Browser::new() 
} 

