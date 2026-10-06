#include "candidate_host.h"
#include "identity.h"
#include <filesystem>
#include <algorithm>
CandidateHost::~CandidateHost() {
    if (packet_) { InterlockedIncrement(&packet_->sequence); packet_->visible = 0; packet_->shutdown = 1; InterlockedIncrement(&packet_->sequence); UnmapViewOfFile(packet_); }
    if (mapping_) CloseHandle(mapping_);
    if (process_) CloseHandle(process_);
}
bool CandidateHost::start(HWND callback) {
    if (process_ && WaitForSingleObject(process_,0) == WAIT_TIMEOUT) return true;
    if (process_) { CloseHandle(process_); process_ = nullptr; }
    if (!mapping_) {
        SECURITY_ATTRIBUTES security{sizeof(security),nullptr,TRUE};
        mapping_ = CreateFileMappingW(INVALID_HANDLE_VALUE,&security,PAGE_READWRITE,0,sizeof(CandidatePacket),nullptr);
        if (!mapping_) return false;
        packet_ = (CandidatePacket*)MapViewOfFile(mapping_,FILE_MAP_WRITE,0,0,sizeof(CandidatePacket));
        if (!packet_) return false;
        packet_->version = 1; packet_->callback = (ULONGLONG)(UINT_PTR)callback; packet_->parent = GetCurrentProcessId();
    }
    wchar_t modulePath[32768]; GetModuleFileNameW(module,modulePath,32768);
    auto root = std::filesystem::path(modulePath).parent_path().parent_path();
    if (root.parent_path().filename() == L"versions") root = root.parent_path().parent_path();
    auto executable = root / L"ui" / L"Shanjie.UI.exe";
    std::wstring command = L"\"" + executable.wstring() + L"\" --candidates " + std::to_wstring((UINT_PTR)mapping_);
    SIZE_T bytes = 0; InitializeProcThreadAttributeList(nullptr,1,0,&bytes);
    std::vector<BYTE> storage(bytes); auto attributes = (LPPROC_THREAD_ATTRIBUTE_LIST)storage.data();
    if (!InitializeProcThreadAttributeList(attributes,1,0,&bytes)) return false;
    UpdateProcThreadAttribute(attributes,0,PROC_THREAD_ATTRIBUTE_HANDLE_LIST,&mapping_,sizeof(mapping_),nullptr,nullptr);
    STARTUPINFOEXW startup{}; startup.StartupInfo.cb = sizeof(startup); startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW; startup.StartupInfo.wShowWindow = SW_HIDE; startup.lpAttributeList = attributes;
    PROCESS_INFORMATION child{};
    BOOL ok = CreateProcessW(executable.c_str(),command.data(),nullptr,nullptr,TRUE,EXTENDED_STARTUPINFO_PRESENT,nullptr,root.c_str(),&startup.StartupInfo,&child);
    DeleteProcThreadAttributeList(attributes);
    if (!ok) { OutputDebugStringW(L"Shanjie: WinUI candidate host could not start.\n"); return false; }
    process_ = child.hProcess; CloseHandle(child.hThread); return true;
}
void CandidateHost::publish(const shanjie::Snapshot& state,const Preferences& preferences,const RECT& caret,UINT dpi,bool visible) {
    if (!packet_) return;
    InterlockedIncrement(&packet_->sequence);
    packet_->visible = visible; packet_->layout = (DWORD)preferences.layout;
    packet_->selected = state.selected; packet_->count = (DWORD)std::min<size_t>(state.candidates.size(),81);
    packet_->first = state.first; packet_->total = state.total;
    packet_->left = caret.left; packet_->bottom = caret.bottom; packet_->top = caret.top; packet_->dpi = dpi;
    packet_->theme = preferences.theme; packet_->fontSize = preferences.fontSize; packet_->columns = state.columns;
    for (DWORD i = 0; i < packet_->count; ++i) wcsncpy_s(packet_->words[i],state.candidates[i].c_str(),_TRUNCATE);
    InterlockedIncrement(&packet_->sequence);
}
void CandidateHost::hide() {
    if (!packet_) return;
    InterlockedIncrement(&packet_->sequence); packet_->visible = 0; InterlockedIncrement(&packet_->sequence);
}
