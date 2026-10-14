#include <windows.h>
#include <fstream>
#include <iterator>
#include <string>
#include <cstdio>
std::string utf8(const std::wstring& value) {
    int count=WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),nullptr,0,nullptr,nullptr);
    std::string out(count,'\0');WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),out.data(),count,nullptr,nullptr);return out;
}
int wmain(int argc,wchar_t** argv) {
    if(argc!=4)return 1;
    auto module=LoadLibraryW(argv[1]);if(!module)return 2;
    auto save=(char* (__cdecl*)(const char*,const char*))GetProcAddress(module,"sj_dictionary_save");
    auto free=(void (__cdecl*)(char*))GetProcAddress(module,"sj_string_free");
    auto error=(const char* (__cdecl*)())GetProcAddress(module,"sj_error");
    if(!save||!free||!error)return 3;
    std::string folder=utf8(argv[2]),name,expected="ㄋㄧˇ\t你\t-0.01\nㄏㄠˇ\t好\t-0.01\n";
    int iterations=_wtoi(argv[3]);
    for(int at=0;at<iterations;++at) {
        char* result=save("ㄋㄧˇ 你\rㄏㄠˇ 好",folder.c_str());
        if(!result){std::printf("ERROR %s\n",error());return 4;}
        if(!name.empty()&&name!=result)return 5;name=result;free(result);
        std::ifstream file(std::wstring(argv[2])+L"/"+std::wstring(name.begin(),name.end()),std::ios::binary);
        std::string actual((std::istreambuf_iterator<char>(file)),{});
        if(actual!=expected)return 6;
    }
    std::printf("PASS %d saves %s\n",iterations,name.c_str());return 0;
}
