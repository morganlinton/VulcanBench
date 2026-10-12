/* P1 proving fixture: a TU that compiles only because the host C compiler's
   default `#include <...>` search list resolves <stdint.h>. With no explicit
   `-I`, vyrec must fall through to the system include defaults. <stdint.h>
   is chosen over <stdio.h> because it is self-contained and an order of
   magnitude smaller after expansion, keeping the GPU lex/parse run cheap
   while still exercising the system search path. */
#include <stdint.h>

int32_t add(int32_t a, int32_t b) {
    return a + b;
}
