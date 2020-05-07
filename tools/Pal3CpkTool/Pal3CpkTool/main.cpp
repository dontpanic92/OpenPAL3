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
    /*if (pDirectoryEntry->iAttrib & CpkFileAttrib_IsDeleted) {
        if (strlen(pDirectoryEntry->lpszName) < 2)
            sprintf_s(pDirectoryEntry->lpszName, sizeof(pDirectoryEntry->lpszName), "deleted_%d", g_deletedCount++);
    }*/
    sprintf_s(absPath, sizeof(absPath), "%s\\%s", rootPath, pDirectoryEntry->lpszName);
    if (pDirectoryEntry->iAttrib & CPKTableFlag_IsDir) {
        createDirectory(absPath);
        for (int i = 0; i < pDirectoryEntry->childs.size(); i++) {
            decompress(cpk, rootPath, pDirectoryEntry->childs[i]);
        }
    } else {
        //open file and decompress it
        if (!strlen(pDirectoryEntry->lpszName))
            return true;
        CPKFile* pFile = cpk->Open(pDirectoryEntry->vCRC, pDirectoryEntry->lpszName);
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

#if 1
    //解压缩功能
    std::string fileBaseName = getBaseFileName(cpkFilePath);
    if (!fileBaseName.length())
        return -1;

    //为输出目录拼接cpk baseName
    saveRootPath.append("\\").append(fileBaseName);
    CPKDirectoryEntry entry;
    printf("正在解析cpk文件结构\n");
    cpk.BuildDirectoryTree(entry);
    printf("=================================\n");
    printf("开始解压...\n");
    printf("=================================\n");
    for (int i = 0; i < entry.childs.size(); i++) {
        CPKDirectoryEntry* pChild = entry.childs[i];
        printf("正在处理: %s => %s\\%s\n", pChild->lpszName, saveRootPath.c_str(), pChild->lpszName);
        decompress(&cpk, saveRootPath.c_str(), pChild);
    }
#else
    //测试zol库的压缩加压缩功能，验证压缩后解压，得到的结果是否完全一致
    CPKFile* pFile = cpk.Open("cbdata\\memoryLogFile.log");
    if (!pFile)
        return -1;

    char* compressBuf = new char[pFile->pRecordEntry->dwPackedSize];
    char* deCompressedBuf = new char[pFile->pRecordEntry->dwOriginSize];
    DWORD dwResultSize = cpk.Compress(compressBuf, pFile->lpMem, pFile->dwFileSize);
    assert(dwResultSize == pFile->pRecordEntry->dwPackedSize);
    //压缩后应该和原始文件内容一致
    if (!memcmp(pFile->lpStartAddress, compressBuf, dwResultSize))
        printf("压缩测试通过！\n");
    else {
        printf("压缩测试通过！\n");
    }
    cpk.DeCompress(deCompressedBuf, compressBuf, dwResultSize);
    printf("解压结果：\n%s\n", deCompressedBuf);
    cpk.Close(pFile);
#endif
    return 0;
}