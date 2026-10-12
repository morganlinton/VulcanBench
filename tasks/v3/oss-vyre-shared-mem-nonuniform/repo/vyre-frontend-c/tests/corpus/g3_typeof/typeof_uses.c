int f(void) {
    int x = 5;
    __typeof__(x) y = x;
    typeof(x + 1) z = 0;
    return y + z;
}
