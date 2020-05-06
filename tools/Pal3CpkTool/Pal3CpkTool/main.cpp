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


bool decompress(CPK* cpk, const char* rootPath, CPKDirectoryEntry* pDirectoryEntry) {
    char absPath[MAX_PATH] = { 0 };
    sprintf_s(absPath, sizeof(absPath), "%s\\%s", rootPath, pDirectoryEntry->lpszName);
    if (pDirectoryEntry->iAttrib & CpkFileAttrib_IsDir) {
        createDirectory(absPath);
        for (int i = 0; i < pDirectoryEntry->childs.size(); i++) {
            decompress(cpk, rootPath, pDirectoryEntry->childs[i]);
        }
    } else {
        //open file and decompress it
        if (!strlen(pDirectoryEntry->lpszName))
            return true;
        CPKFile* pFile = cpk->Open(pDirectoryEntry->lpszName);
        if (!pFile)
            return false;
        assert(pFile);
        HANDLE hFile = CreateFileA(absPath, GENERIC_WRITE, FILE_SHARE_WRITE, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
        if (hFile == INVALID_HANDLE_VALUE) {
            DWORD dwError = GetLastError();
            assert(false);
        }
        DWORD dwBytesWritten;
        BOOL bOk = WriteFile(hFile, pFile->pDest, pFile->originalSize, &dwBytesWritten, NULL);
        assert(bOk && dwBytesWritten == pFile->originalSize);
        CloseHandle(hFile);
        cpk->Close(pFile);
    }
    return true;
}

//获取cpk文件的baseName
std::string getBaseFileName(const std::string cpkFullPath)
{
    auto pos1 = cpkFullPath.find(".cpk");
    if (pos1 == std::string::npos)
        return "";
    auto pos2 = cpkFullPath.find_last_of("\\");
    if (pos2 == std::string::npos)
        pos2 = cpkFullPath.find_last_not_of("/");
    if (pos2 == std::string::npos) {
        return "";
    }
    pos2 += 1;
    std::string fileBaseName = cpkFullPath.substr(pos2, pos1 - pos2);
    return fileBaseName;
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

    std::string fileBaseName = getBaseFileName(cpkFilePath);
    if (!fileBaseName.length())
        return -1;

    //为输出目录拼接cpk baseName
    saveRootPath.append("\\").append(fileBaseName);
    CPKDirectoryEntry entry;
    cpk.buildDirectoryTree(entry);
    for (int i = 0; i < entry.childs.size(); i++) {
        CPKDirectoryEntry* pChild = entry.childs[i];
        decompress(&cpk, saveRootPath.c_str(), pChild);
    }
    return 0;
}