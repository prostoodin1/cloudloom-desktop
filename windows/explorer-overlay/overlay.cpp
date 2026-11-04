#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shlobj.h>
#include <atomic>
#include <cwchar>
#include <new>

namespace {
constexpr CLSID pendingClass{0x7F0A4686, 0x1A58, 0x42F1, {0xA1, 0x04, 0x17, 0x88, 0x02, 0xE5, 0x13, 0x41}};
constexpr CLSID completeClass{0x7F0A4686, 0x1A58, 0x42F1, {0xA1, 0x04, 0x17, 0x88, 0x02, 0xE5, 0x13, 0x42}};
constexpr wchar_t statusKey[] = L"Software\\Cloudloom\\ExplorerSync";
std::atomic<unsigned> liveObjects{0};
std::atomic<unsigned> serverLocks{0};
HMODULE module = nullptr;

class Overlay final : public IShellIconOverlayIdentifier {
    std::atomic<ULONG> refs{1};
    const DWORD state;

public:
    explicit Overlay(DWORD value) : state(value) { ++liveObjects; }
    ~Overlay() { --liveObjects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid != IID_IUnknown && iid != IID_IShellIconOverlayIdentifier) return E_NOINTERFACE;
        *out = static_cast<IShellIconOverlayIdentifier*>(this);
        AddRef();
        return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override {
        const auto count = --refs;
        if (!count) delete this;
        return count;
    }
    HRESULT STDMETHODCALLTYPE GetOverlayInfo(PWSTR path, int capacity, int* index, DWORD* flags) override {
        if (!path || !index || !flags || capacity <= 0) return E_INVALIDARG;
        const DWORD length = GetModuleFileNameW(module, path, static_cast<DWORD>(capacity));
        if (!length || length >= static_cast<DWORD>(capacity)) return E_FAIL;
        *index = state == 1 ? 0 : 1;
        *flags = ISIOI_ICONFILE | ISIOI_ICONINDEX;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetPriority(int* priority) override {
        if (!priority) return E_POINTER;
        *priority = state == 1 ? 40 : 60;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE IsMemberOf(PCWSTR path, DWORD attributes) override {
        if (!path || (attributes & FILE_ATTRIBUTE_DIRECTORY)) return S_FALSE;
        const auto length = wcsnlen_s(path, 32768);
        if (length <= 3 || length > 16000 || path[1] != L':' || path[2] != L'\\') return S_FALSE;
        const wchar_t letter = path[0];
        if (!((letter >= L'D' && letter <= L'Z') || (letter >= L'd' && letter <= L'z'))) return S_FALSE;

        // No disk or network calls for the file Explorer is displaying.
        ULONGLONG record = 0;
        DWORD size = sizeof(record);
        if (RegGetValueW(HKEY_CURRENT_USER, statusKey, path, RRF_RT_REG_QWORD,
                         nullptr, &record, &size) != ERROR_SUCCESS || size != sizeof(record)) return S_FALSE;
        FILETIME time{};
        GetSystemTimeAsFileTime(&time);
        ULARGE_INTEGER ticks{};
        ticks.LowPart = time.dwLowDateTime;
        ticks.HighPart = time.dwHighDateTime;
        constexpr ULONGLONG epoch = 11644473600ULL;
        const auto now = ticks.QuadPart / 10000000ULL - epoch;
        const auto expires = record >> 2;
        return ((record & 3) == state && expires >= now && expires - now <= 30)
            ? S_OK : S_FALSE;
    }
};

class Factory final : public IClassFactory {
    std::atomic<ULONG> refs{1};
    const DWORD state;
public:
    explicit Factory(DWORD value) : state(value) { ++liveObjects; }
    ~Factory() { --liveObjects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid != IID_IUnknown && iid != IID_IClassFactory) return E_NOINTERFACE;
        *out = static_cast<IClassFactory*>(this);
        AddRef();
        return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++refs; }
    ULONG STDMETHODCALLTYPE Release() override {
        const auto count = --refs;
        if (!count) delete this;
        return count;
    }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID iid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (outer) return CLASS_E_NOAGGREGATION;
        auto overlay = new (std::nothrow) Overlay(state);
        if (!overlay) return E_OUTOFMEMORY;
        const auto result = overlay->QueryInterface(iid, out);
        overlay->Release();
        return result;
    }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override {
        if (lock) ++serverLocks;
        else --serverLocks;
        return S_OK;
    }
};
}

BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) module = instance;
    return TRUE;
}
STDAPI DllGetClassObject(REFCLSID id, REFIID iid, void** out) {
    if (!out) return E_POINTER;
    *out = nullptr;
    const DWORD state = id == pendingClass ? 1 : id == completeClass ? 2 : 0;
    if (!state) return CLASS_E_CLASSNOTAVAILABLE;
    auto factory = new (std::nothrow) Factory(state);
    if (!factory) return E_OUTOFMEMORY;
    const auto result = factory->QueryInterface(iid, out);
    factory->Release();
    return result;
}
STDAPI DllCanUnloadNow() {
    return liveObjects == 0 && serverLocks == 0 ? S_OK : S_FALSE;
}
