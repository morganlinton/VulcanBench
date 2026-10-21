/* G15 proving fixture  -  GNU keyword aliases __inline, __restrict, __volatile__
 * must lex to the same token kinds as the bare keywords.
 */

__inline static int foo(int *__restrict p) {
    __volatile__ int x = *p;
    return x;
}

int main(void) {
    int a = 42;
    return foo(&a);
}
