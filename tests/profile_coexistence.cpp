// Calls real release DLLs with an isolated profile and no system notifications.
#include <windows.h>
#include <filesystem>
#include <fstream>
#include <cstdio>
#include <string>
#include <stdexcept>
struct Reader {
    HMODULE module;
    char* (*load)();
    void (*release)(char*);
    bool (*save)(const char*);
    Reader(const wchar_t* path) : module(LoadLibraryW(path)) {
        if (!module) throw std::runtime_error("DLL load failed");
        load = reinterpret_cast<char* (*)()>(GetProcAddress(module,"sj_profile_load_text"));
        release = reinterpret_cast<void (*)(char*)>(GetProcAddress(module,"sj_string_free"));
        save = reinterpret_cast<bool (*)(const char*)>(GetProcAddress(module,"sj_profile_save"));
        if (!load || !release || !save) throw std::runtime_error("Profile ABI missing");
    }
    ~Reader() { FreeLibrary(module); }
    std::string read() {
        char* text = load();
        if (!text) throw std::runtime_error("Profile read failed");
        std::string result(text); release(text); return result;
    }
};
std::string contents(const std::filesystem::path& path) {
    std::ifstream file(path,std::ios::binary);
    return {std::istreambuf_iterator<char>(file),std::istreambuf_iterator<char>()};
}
void require(bool condition, const char* message) { if (!condition) throw std::runtime_error(message); }
int wmain(int argc,wchar_t** argv) {
    if (argc != 4 && argc != 5) return 2;
    try {
        const std::string oldSchema = argc==5 && std::wstring(argv[4])==L"6" ? "schema=6" : "schema=5";
        std::filesystem::path root(argv[3]);
        std::filesystem::create_directories(root / L"Shanjie");
        auto path = root / L"Shanjie/profile.ini";
        require(!std::filesystem::exists(path),"Fixture must be new");
        SetEnvironmentVariableW(L"LOCALAPPDATA",argv[3]);
        SetEnvironmentVariableW(L"SHANJIE_TEST_PROFILE",L"1");
        std::ofstream(path,std::ios::binary) << "[profile]\nschema=5\ninteraction=windows\n[input]\nkeyboard=1\nplain_punctuation=1\ndictionary=fixture.tsv\n[appearance]\nfont_size=24\n";
        Reader old(argv[1]),fixed(argv[2]);
        old.read(); // The old reader may fill its own missing defaults once.
        const auto baseline=contents(path);
        for (int i=0;i<32;++i) {
            require(old.read().find(oldSchema)!=std::string::npos,"Old reader schema changed");
            auto draft=fixed.read();
            require(draft.find("schema=6")!=std::string::npos,"Fixed reader did not migrate in memory");
            require(draft.find("dictionary=fixture.tsv")!=std::string::npos,"Dictionary preference changed");
            require(draft.find("font_size=24")!=std::string::npos,"Appearance preference changed");
            require(contents(path)==baseline,"Alternating readers changed disk profile");
        }
        std::puts("PASS: 32 old/new read pairs leave the disk profile unchanged");
        const auto draft=fixed.read();
        const auto backup=root / L"Shanjie/profile.ini.schema5.bak";
        const auto expectedBackup=std::filesystem::exists(backup) ? contents(backup) : baseline;
        require(fixed.save(draft.c_str()),"Explicit save failed");
        require(contents(path).find("schema=6")!=std::string::npos,"Explicit save did not persist schema 6");
        require(contents(backup)==expectedBackup,"First legacy backup was not preserved");
        old.read(); // Deliberately model a surviving old process reading again.
        const auto afterOld=contents(path);
        require(afterOld.find(oldSchema)!=std::string::npos,"Old writer behavior missing from fixture");
        for (int i=0;i<32;++i) { fixed.read(); old.read(); require(contents(path)==afterOld,"Readers resumed schema ping-pong"); }
        std::puts("PASS: explicit save retains backup; later old reads cannot restart schema ping-pong");
        return 0;
    } catch(const std::exception& error) { std::printf("FAIL: %s\n",error.what()); return 1; }
}
