type outcome =
  | Succeeded of string 
  | Cancelled 
  | Failed of string 
type completion =
  | Committed of outcome 
  | Journal_failed of string 
  | Aborted 
