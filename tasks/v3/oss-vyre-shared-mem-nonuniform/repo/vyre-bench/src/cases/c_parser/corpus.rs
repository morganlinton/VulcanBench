pub(super) const LINUX_DRIVER_TU: &str = r#"
typedef unsigned long ulong_t;

struct file_operations {
    int (*read)(void *f, void *buf, ulong_t len);
    void (*release)(void *f);
};

struct file {
    struct file_operations *f_op;
    int f_flags;
};

static int demo_read(void *f, void *buf, ulong_t len)
{
    (void)f;
    (void)buf;
    (void)len;
    return 0;
}

static void demo_release(void *f)
{
    (void)f;
}

static struct file_operations demo_fops = {
    .read = demo_read,
    .release = demo_release,
};

static int linux_fop_open(struct file *filp)
{
    struct file local = (struct file){
        .f_op = &demo_fops,
        .f_flags = 0,
    };
    int bump = local.f_flags + 3;
    if (filp && filp->f_op && filp->f_op->read)
        bump += filp->f_op->read(filp, 0, 0);
    return bump;
}
"#;

pub(super) struct CParserPrepared {
    pub(super) source: String,
}

pub(super) fn linux_driver_corpus(workloads: usize) -> String {
    let mut source = String::with_capacity(LINUX_DRIVER_TU.len().saturating_mul(workloads));
    source.push_str("typedef unsigned long ulong_t;\n");
    source.push_str("struct file_operations { int (*read)(void *f, void *buf, ulong_t len); void (*release)(void *f); };\n");
    source.push_str("struct file { struct file_operations *f_op; int f_flags; };\n");
    for idx in 0..workloads {
        source.push_str(&format!(
            r#"
static int demo_read_{idx}(void *f, void *buf, ulong_t len)
{{
    (void)f;
    (void)buf;
    (void)len;
    return {idx};
}}

static void demo_release_{idx}(void *f)
{{
    (void)f;
}}

static struct file_operations demo_fops_{idx} = {{
    .read = demo_read_{idx},
    .release = demo_release_{idx},
}};

static int linux_fop_open_{idx}(struct file *filp)
{{
    struct file local = (struct file){{
        .f_op = &demo_fops_{idx},
        .f_flags = {idx},
    }};
    int bump = local.f_flags + {idx};
    if (filp && filp->f_op && filp->f_op->read)
        bump += filp->f_op->read(filp, 0, 0);
    return bump;
}}
"#
        ));
    }
    source
}
