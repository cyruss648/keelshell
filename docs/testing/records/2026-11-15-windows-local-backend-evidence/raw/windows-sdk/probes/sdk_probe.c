#include <windows.h>
#include <stdlib.h>
#include <stdint.h>
#include <intrin.h>

_Static_assert(sizeof(void *) == 8, "The probe must target Windows x64.");
_Static_assert(sizeof(wchar_t) == 2, "The probe must use the Windows ABI.");

__declspec(dllexport) uint64_t sdk_probe(void) {
    LARGE_INTEGER counter;
    void *allocation = malloc(32);
    BOOL observed = QueryPerformanceCounter(&counter);
    free(allocation);
    return observed ? (uint64_t)counter.QuadPart : 0;
}
