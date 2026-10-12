// Integration test module for the containing Vyre package.

pub(crate) const SOURCE: &str = "int main(){return 0;}\n";

pub(crate) const KERNEL_LIBC_SHAPED_SOURCE: &str = r#"
typedef unsigned long size_t;
typedef int (*probe_fn)(const char *name, unsigned long flags);

struct file_ops {
    probe_fn open;
    int (*close)(void *ctx);
};

struct file_handle {
    struct file_ops *ops;
    void *ctx;
};

static int device_open(const char *name, unsigned long flags);

static int device_open(const char *name, unsigned long flags)
{
    int state = 0;
    struct file_handle handle = { 0 };
retry:
    switch (flags) {
    case 0:
        state = flags ? 1 : 0;
        break;
    default:
        state += flags;
        goto retry_done;
    }

    if (state < 0)
        goto retry;

    if (handle.ops && handle.ops->open)
        state += handle.ops->open(name, flags);

retry_done:
    return state;
}
"#;

pub(crate) const AST_PARSER_GAP_SOURCE: &str = r#"
typedef int alias;

int first_shadow(int alias)
{
    return alias;
}

int second_restore(alias *out)
{
    *out = 1;
    return *out;
}

struct holder { int field; };
char banner[] = "vyre";

int container_shape(struct holder *p)
{
    return ((struct holder *)((char *)(p) - 0))->field;
}

int parser_gap_suite(int n)
{
    int sum = 0;
    for (int loop_i = 0; loop_i < n; loop_i++)
        sum = sum + loop_i;
    do {
        observe_call(n);
    } while (0);
    return sum + banner[0];
}
"#;
