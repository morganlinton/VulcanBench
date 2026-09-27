#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define LEDGER_MAX_RECORDS 256
#define LEDGER_NAME_MAX 32
enum ledger_status{LEDGER_OK=0,LEDGER_E_FIELDS,LEDGER_E_KIND};
struct record{char account[LEDGER_NAME_MAX];char kind[LEDGER_NAME_MAX];long long amount_cents;};
struct balance{char account[LEDGER_NAME_MAX];long long cents;};
static bool copy_trimmed(char d[LEDGER_NAME_MAX],const char*s,const char*e){while(s<e&&isspace((unsigned char)*s))s++;while(e>s&&isspace((unsigned char)e[-1]))e--;size_t n=(size_t)(e-s);if(n>=LEDGER_NAME_MAX)return false;memcpy(d,s,n);d[n]=0;return true;}
static bool parse_amount(long long*v,const char*s,const char*e){char b[LEDGER_NAME_MAX];char*r;if(!copy_trimmed(b,s,e))return false;*v=strtoll(b,&r,10);return r!=b&&!*r;}
enum ledger_status parse_records(const char*t,struct record*o,size_t*c,int*l){size_t n=0;int i=0;
for(const char*x=t;*x;){const char*e=x+strcspn(x,"\n"),*s=x;i++;while(s<e&&isspace((unsigned char)*s))s++;
if(s<e&&*s!='#'){const char*f[4]={x};size_t k=1;for(const char*p=x;p<e;p++){if(*p!=',')continue;if(k<4)f[k]=p+1;k++;}
if(k!=4||n==LEDGER_MAX_RECORDS||!copy_trimmed(o[n].account,f[1],f[2]-1)||!copy_trimmed(o[n].kind,f[2],f[3]-1)||!parse_amount(&o[n].amount_cents,f[3],e)){*l=i;return LEDGER_E_FIELDS;}n++;}
x=*e=='\n'?e+1:e;}*c=n;return LEDGER_OK;}
enum ledger_status fee_cents(const char*k,long long a,long long*f){
if(!strcmp(k,"card")){long long s=a*29+500;*f=s/1000-(s%1000<0)+30;return LEDGER_OK;}if(!strcmp(k,"bank")){*f=25;return LEDGER_OK;}if(!strcmp(k,"cash")){*f=0;return LEDGER_OK;}return LEDGER_E_KIND;}
enum ledger_status balances(const struct record*r,size_t c,struct balance*o,size_t*m){size_t n=0;
for(size_t i=0;i<c;i++){long long f;enum ledger_status s=fee_cents(r[i].kind,r[i].amount_cents,&f);if(s!=LEDGER_OK)return s;size_t j=0;while(j<n&&strcmp(o[j].account,r[i].account))j++;
if(j==n){snprintf(o[n].account,sizeof o[n].account,"%s",r[i].account);o[n++].cents=0;}o[j].cents+=r[i].amount_cents-f;}*m=n;return LEDGER_OK;}
static void format_cents(long long c,char*o,size_t z){long long m=llabs(c);snprintf(o,z,c<0?"(%lld.%02lld)":"%lld.%02lld",m/100,m%100);}
static int compare_accounts(const void*a,const void*b){return strcmp(((const struct balance*)a)->account,((const struct balance*)b)->account);}
void render(const struct balance*b,size_t c,char*o,size_t z){struct balance s[LEDGER_MAX_RECORDS];size_t u=0;if(c>LEDGER_MAX_RECORDS)c=LEDGER_MAX_RECORDS;if(z>0)o[0]=0;
memcpy(s,b,c*sizeof*b);qsort(s,c,sizeof*s,compare_accounts);
for(size_t i=0;i<c&&u<z;i++){char d[32];format_cents(s[i].cents,d,sizeof d);int w=snprintf(o+u,z-u,"%s%s  %s",i>0?"\n":"",s[i].account,d);if(w<0)break;u+=(size_t)w;}}
