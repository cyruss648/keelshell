/* Diagnostic only: observe the unmodified external peer's original tcflush. */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

static int observed_tcflush(int fd, int selector)
{
    int incoming_errno = errno;
    int queued = -1;
    int queue_result = ioctl(fd, TIOCOUTQ, &queued);
    errno = incoming_errno;
    int result = tcflush(fd, selector);
    int saved_errno = errno;
    const char *path = getenv("KEELSHELL_DIAGNOSTIC_FLUSH_LOG");
    if (path) {
        FILE *log = fopen(path, "a");
        if (log) {
            fprintf(log,
                    "{\"pid\":%d,\"fd\":%d,\"selector\":%d,"
                    "\"queued_output_before\":%d,\"queue_result\":%d,"
                    "\"flush_result\":%d}\n",
                    getpid(), fd, selector, queued, queue_result, result);
            fclose(log);
        }
    }
    errno = saved_errno;
    return result;
}

__attribute__((used, section("__DATA,__interpose")))
static const struct {
    const void *replacement;
    const void *original;
} interpose_flush = {(const void *)observed_tcflush, (const void *)tcflush};
