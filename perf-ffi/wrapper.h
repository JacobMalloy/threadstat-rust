#include <linux/hw_breakpoint.h> /* Definition of HW_* constants */
#include <linux/perf_event.h>    /* Definition of PERF_* constants */
#include <sys/syscall.h>         /* Definition of SYS_* constants */
#include <unistd.h>

#include <perfmon/pfmlib.h>
#include <perfmon/pfmlib_perf_event.h>

/* bindgen cannot evaluate complex macro chains like _IOR(...), so expose
   the ioctl request codes as typed constants it can handle. */
static const unsigned long PERF_EVENT_IOC_ID_CONST = PERF_EVENT_IOC_ID;
