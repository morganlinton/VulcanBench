#include <cstdlib>
#include <iomanip>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>
struct Record{std::string account;std::string kind;long long amount_cents;};
std::string trim(const std::string&s){auto b=s.find_first_not_of(" \t\r\n\f\v");if(b==std::string::npos)return "";
    return s.substr(b,s.find_last_not_of(" \t\r\n\f\v")-b+1);}
std::vector<std::string> split_fields(const std::string&s){std::vector<std::string> f;std::size_t b=0,e=s.find(',');
    while(e!=std::string::npos){f.push_back(s.substr(b,e-b));b=e+1;e=s.find(',',b);}f.push_back(s.substr(b));return f;}
std::vector<Record> parse_records(const std::string&t){std::vector<Record> r;std::istringstream s(t);std::string l;
    for(int i=1;std::getline(s,l);++i){l=trim(l);if(l.empty()||l[0]=='#')continue;auto f=split_fields(l);
        if(f.size()!=4)throw std::invalid_argument("line "+std::to_string(i)+": expected 4 fields, got "+std::to_string(f.size()));
        r.push_back({trim(f[1]),trim(f[2]),std::stoll(f[3])});}return r;}
long long fee_cents(const std::string&k,long long a){if(k=="card"){long long p=a*29+500;return p/1000-(p%1000<0)+30;}
    if(k=="bank")return 25;if(k=="cash")return 0;throw std::invalid_argument("unknown kind: "+k);}
std::map<std::string,long long> balances(const std::vector<Record>&r){std::map<std::string,long long> b;
    for(auto&[a,k,c]:r)b[a]+=c-fee_cents(k,c);return b;}
std::string format_cents(long long c){std::ostringstream o;o<<std::abs(c)/100<<'.'<<std::setw(2)<<std::setfill('0')<<std::abs(c)%100;return c<0?"("+o.str()+")":o.str();}
std::vector<std::string> render(const std::map<std::string,long long>&b){std::vector<std::string> l;for(auto&[a,c]:b)l.push_back(a+"  "+format_cents(c));return l;}
