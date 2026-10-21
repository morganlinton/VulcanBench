/* Vendored stub of Linux kernel `security/selinux/include/classmap.h`
 * sufficient for scripts/selinux/mdp/mdp.c to parse against the corpus.
 * The real header pulls in capability/socket headers and defines a
 * massive `secclass_map[]`. This stub provides only the structure
 * needed for the parser to fold the include without errors.
 */
#ifndef __VYRE_VENDOR_SELINUX_CLASSMAP_H
#define __VYRE_VENDOR_SELINUX_CLASSMAP_H

#define COMMON_FILE_SOCK_PERMS "ioctl", "read", "write", "create", \
		"getattr", "setattr", "lock", "relabelfrom", "relabelto", "append"

struct security_class_mapping secclass_map[] = {
	{ "security", { "compute_av", "compute_create", "compute_member",
			"check_context", "load_policy", "compute_relabel",
			NULL } },
	{ "process", { "fork", "transition", "sigchld", "sigkill", "sigstop",
			"signull", "signal", NULL } },
	{ "system", { "ipc_info", "syslog_read", "syslog_mod", "syslog_console",
			"module_request", NULL } },
	{ "capability", { "chown", "dac_override", "dac_read_search",
			NULL } },
	{ NULL }
};

#endif /* __VYRE_VENDOR_SELINUX_CLASSMAP_H */
