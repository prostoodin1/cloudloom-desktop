#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shlobj.h>
#include <shellapi.h>
#include <cassert>
#include <iostream>

int wmain(int argc, wchar_t** argv) {
    assert(argc == 2);
    HMODULE dll = LoadLibraryW(argv[1]);
    assert(dll);
    using GetClass = HRESULT (WINAPI*)(REFCLSID, REFIID, void**);
    auto getClass = reinterpret_cast<GetClass>(GetProcAddress(dll, "DllGetClassObject"));
    assert(getClass);

    wchar_t keyName[100];
    swprintf_s(keyName, L"Software\\CloudloomOverlayTest-%lu", GetCurrentProcessId());
    HKEY sandbox, statuses;
    assert(RegCreateKeyExW(HKEY_CURRENT_USER, keyName, 0, nullptr, REG_OPTION_VOLATILE,
                          KEY_ALL_ACCESS, nullptr, &sandbox, nullptr) == ERROR_SUCCESS);
    assert(RegOverridePredefKey(HKEY_CURRENT_USER, sandbox) == ERROR_SUCCESS);
    assert(RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Cloudloom\\ExplorerSync", 0, nullptr,
                          REG_OPTION_VOLATILE, KEY_ALL_ACCESS, nullptr, &statuses, nullptr) == ERROR_SUCCESS);
    FILETIME fileTime;
    GetSystemTimeAsFileTime(&fileTime);
    ULARGE_INTEGER ticks;
    ticks.HighPart = fileTime.dwHighDateTime;
    ticks.LowPart = fileTime.dwLowDateTime;
    const auto now = ticks.QuadPart / 10000000ULL - 11644473600ULL;
    for (DWORD state : {1, 2}) {
        const CLSID id{0x7F0A4686, 0x1A58, 0x42F1, {0xA1, 0x04, 0x17, 0x88, 0x02, 0xE5, 0x13, BYTE(0x40 + state)}};
        IClassFactory* factory = nullptr;
        assert(getClass(id, IID_IClassFactory, reinterpret_cast<void**>(&factory)) == S_OK);
        IShellIconOverlayIdentifier* overlay = nullptr;
        assert(factory->CreateInstance(nullptr, IID_IShellIconOverlayIdentifier, reinterpret_cast<void**>(&overlay)) == S_OK);
        factory->Release();
        wchar_t icon[MAX_PATH];
        int index;
        DWORD flags;
        assert(overlay->GetOverlayInfo(icon, MAX_PATH, &index, &flags) == S_OK && index == static_cast<int>(state) - 1);
        HICON smallIcon = nullptr;
        assert(ExtractIconExW(icon, index, nullptr, &smallIcon, 1) == 1 && smallIcon);
        DestroyIcon(smallIcon);
        auto record = ((now + 10) << 2) | state;
        constexpr auto file = L"Z:\\Folder\\Video.mp4";
        assert(RegSetValueExW(statuses, file, 0, REG_QWORD, reinterpret_cast<const BYTE*>(&record), sizeof(record)) == ERROR_SUCCESS);
        assert(overlay->IsMemberOf(L"z:\\folder\\video.mp4", FILE_ATTRIBUTE_NORMAL) == S_OK);
        assert(overlay->IsMemberOf(file, FILE_ATTRIBUTE_DIRECTORY) == S_FALSE);
        assert(overlay->IsMemberOf(L"Y:\\Folder\\Video.mp4", FILE_ATTRIBUTE_NORMAL) == S_FALSE);
        record = ((now - 1) << 2) | state;
        assert(RegSetValueExW(statuses, file, 0, REG_QWORD, reinterpret_cast<const BYTE*>(&record), sizeof(record)) == ERROR_SUCCESS);
        assert(overlay->IsMemberOf(file, FILE_ATTRIBUTE_NORMAL) == S_FALSE);
        overlay->Release();
    }
    RegCloseKey(statuses);
    RegDeleteKeyW(HKEY_CURRENT_USER, L"Software\\Cloudloom\\ExplorerSync");
    RegDeleteKeyW(HKEY_CURRENT_USER, L"Software\\Cloudloom");
    RegDeleteKeyW(HKEY_CURRENT_USER, L"Software");
    RegOverridePredefKey(HKEY_CURRENT_USER, nullptr);
    RegCloseKey(sandbox);
    RegDeleteKeyW(HKEY_CURRENT_USER, keyName);
    FreeLibrary(dll);
    std::cout << "PASS: both native icons, file matching and expiry\n";
}
