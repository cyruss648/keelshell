#include <windows.h>
#include <cstdlib>
#include <cstdint>
#include <vector>

static_assert(sizeof(void *) == 8, "The probe must target Windows x64.");
static_assert(sizeof(wchar_t) == 2, "The probe must use the Windows ABI.");

std::uint64_t sdk_cpp_probe() {
    std::vector<std::uint64_t> values{sizeof(HANDLE), sizeof(DWORD)};
    return values.front();
}
