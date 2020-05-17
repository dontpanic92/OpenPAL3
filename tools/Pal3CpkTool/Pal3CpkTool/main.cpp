// Pal3CpkTool.cpp : 此文件包含 "main" 函数。程序执行将在此处开始并结束。
//

#include <iostream>
#include <fstream>
#include <windows.h>
#include <io.h>
#include <direct.h> 
#include <cassert>
#include "cpk.h"

#ifdef WIN32
#define ACCESS(fileName,accessMode) _access(fileName,accessMode)
#define MKDIR(path) _mkdir(path)
#else
#define ACCESS(fileName,accessMode) access(fileName,accessMode)
#define MKDIR(path) mkdir(path,S_IRWXU | S_IRWXG | S_IROTH | S_IXOTH)
#endif

int32_t createDirectory(const std::string &directoryPath)
{
    uint32_t dirPathLen = directoryPath.length();
    if (dirPathLen > MAX_PATH) {
        return -1;
    }
    char tmpDirPath[MAX_PATH] = { 0 };
    for (uint32_t i = 0; i < dirPathLen; ++i) {
        tmpDirPath[i] = directoryPath[i];
        if (tmpDirPath[i] == '\\' || tmpDirPath[i] == '/') {
            if (ACCESS(tmpDirPath, 0) != 0) {
                int32_t ret = MKDIR(tmpDirPath);
                if (ret != 0) {
                    return ret;
                }
            }
        }
    }
    if (ACCESS(directoryPath.c_str(), 0) != 0) {
        int32_t ret = MKDIR(directoryPath.c_str());
        if (ret != 0) {
            return ret;
        }
    }

    return 0;
}

//static int g_deletedCount = 0;
bool decompress(CPK* cpk, const char* rootPath, CPKDirectoryEntry* pDirectoryEntry) {
    char absPath[MAX_PATH] = { 0 };
    sprintf_s(absPath, sizeof(absPath), "%s\\%s", rootPath, pDirectoryEntry->lpszName);
    if (pDirectoryEntry->dwFlag & CPKTableFlag_IsDir) {
        createDirectory(absPath);
        for (int i = 0; i < pDirectoryEntry->childs.size(); i++) {
            decompress(cpk, rootPath, pDirectoryEntry->childs[i]);
        }
    } else {
        //open file and decompress it
        if (!strlen(pDirectoryEntry->lpszName))
            return true;
        CPKFile* pFile = cpk->Open(pDirectoryEntry->dwCRC, pDirectoryEntry->lpszName);
        if (!pFile)
            return false;
        assert(pFile);
        HANDLE hFile = CreateFileA(absPath, GENERIC_WRITE, FILE_SHARE_WRITE, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
        if (hFile == INVALID_HANDLE_VALUE) {
            DWORD dwError = GetLastError();
            assert(false);
        }
        DWORD dwBytesWritten;
        BOOL bOk = WriteFile(hFile, pFile->lpMem, pFile->dwFileSize, &dwBytesWritten, NULL);
        assert(bOk && dwBytesWritten == pFile->dwFileSize);
        CloseHandle(hFile);
        cpk->Close(pFile);
    }
    return true;
}

//CPK解压工具 cpk文件路径 解压路径
int main(int argc, char** argv)
{
    if (argc < 3) {
        printf("argument too less!");
        return -1;
    }

    std::string cpkFilePath = argv[1];
    std::string saveRootPath = argv[2];

    createDirectory(saveRootPath);

    CPK cpk;
    bool bOk = cpk.Load(cpkFilePath.c_str());
    if (!bOk)
        return -1;

    CPKDirectoryEntry entry;
    printf("=================================\n");
    printf("开始处理： %s\n", cpkFilePath.c_str());
    printf("正在解析cpk文件结构\n");
    cpk.BuildDirectoryTree(entry);
    printf("开始解压...\n");
    for (int i = 0; i < entry.childs.size(); i++) {
        CPKDirectoryEntry* pChild = entry.childs[i];
        printf("正在处理: %s => %s\\%s\n", pChild->lpszName, saveRootPath.c_str(), pChild->lpszName);
        decompress(&cpk, saveRootPath.c_str(), pChild);
    }
    printf("=================================\n");
    printf("\n");

    return 0;
}