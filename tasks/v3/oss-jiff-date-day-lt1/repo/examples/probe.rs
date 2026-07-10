fn main() {
  use jiff::civil::Date;
  println!("day0  is_ok={}", Date::new(1998,1,0).is_ok());
  println!("dayneg is_ok={}", Date::new(1998,1,-1).is_ok());
  println!("feb29_2004 is_ok={}", Date::new(2004,2,29).is_ok());
  println!("feb29_1998 is_err={}", Date::new(1998,2,29).is_err());
}
