type quote = {
  id: int64 ;
  label: string ;
  urgent: bool }
let v1 =
  Schema.Record
    ((Field
        (1, Int64, Required, (fun q -> q.id),
          (Field (2, Text, Required, (fun q -> q.label), Nil)))),
      (fun (id, (label, ())) -> { id; label; urgent = false }))
let v2 =
  Schema.Record
    ((Field
        (3, Bool, (Default false), (fun q -> q.urgent),
          (Field
             (2, Text, Required, (fun q -> q.label),
               (Field (1, Int64, Required, (fun q -> q.id), Nil)))))),
      (fun (urgent, (label, (id, ()))) -> { id; label; urgent }))
let batch = Schema.List v2
