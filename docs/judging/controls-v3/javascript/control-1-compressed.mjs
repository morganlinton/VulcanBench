export function parseRecords(t){const r=[];
  for(let[i,l]of t.split("\n").entries()){l=l.trim();if(!l||l[0]==="#")continue;
    const f=l.split(",");if(f.length!==4)throw new Error(`line ${i+1}: expected 4 fields, got ${f.length}`);
    r.push({account:f[1].trim(),kind:f[2].trim(),amountCents:Number.parseInt(f[3],10)});}
  return r;}
export function feeCents(k,a){if(k==="card")return Math.floor((a*29+500)/1000)+30;
  if(k==="bank")return 25;if(k==="cash")return 0;
  throw new Error(`unknown kind: ${k}`);}
export function balances(r){const b=new Map();
  for(const{account:a,kind:k,amountCents:c}of r)b.set(a,(b.get(a)??0)+c-feeCents(k,c));return b;}
function formatCents(c){const d=`${Math.floor(Math.abs(c)/100)}.${String(Math.abs(c)%100).padStart(2,"0")}`;return c<0?`(${d})`:d;}
export function render(b){return[...b.keys()].sort().map(a=>`${a}  ${formatCents(b.get(a))}`);}
