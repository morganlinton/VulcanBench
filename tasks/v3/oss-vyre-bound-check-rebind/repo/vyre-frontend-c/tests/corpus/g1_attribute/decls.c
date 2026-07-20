// Function with leading attribute
__attribute__((noreturn)) void die1(void) { while (1); }

// Function with trailing attribute
void die2(void) __attribute__((noreturn)) { while (1); }

// Function with attribute between type and name
int __attribute__((noreturn)) die3(void) { while (1); }

// Variable with leading attribute
__attribute__((aligned(8))) int global1;

// Variable with trailing attribute
int global2 __attribute__((aligned(8)));

// Struct with leading attribute
__attribute__((packed)) struct S1 { int a; };

// Struct field with trailing attribute
struct S2 { int x __attribute__((aligned(4))); };

// Struct field with leading attribute
struct S3 { __attribute__((aligned(4))) int y; };

// Enum with attribute
enum E1 { E1A __attribute__((unused)), E1B };

// format attribute
__attribute__((format(printf, 1, 2))) void log1(const char *fmt, ...) {}

// section attribute
__attribute__((section(".data"))) int global3 = 42;

// visibility attribute
__attribute__((visibility("hidden"))) void hidden_fn(void) {}

// deprecated attribute
__attribute__((deprecated)) void old_fn(void) {}

// unused attribute
__attribute__((unused)) int global4;

int main(void) { return 0; }
