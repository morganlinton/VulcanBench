/* P1 negative twin: a header that exists nowhere on the host. vyrec must
   reject this without falling back silently to "directive lane" pass-through;
   the rejection must reach the host preprocessor surface as an error. */
#include <this_header_does_not_exist_88e5f1c2.h>

int main(void) { return 0; }
