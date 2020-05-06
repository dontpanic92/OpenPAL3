// Pal3CpkTool.cpp : 此文件包含 "main" 函数。程序执行将在此处开始并结束。
//

#include <iostream>
#include <fstream>
#include <windows.h>
#include "cpk.h"

int main()
{
    //测试CPK接口
    std::string filePath = R"(E:\PAL3\basedata\basedata.cpk)";
    CPK cpk;
    bool bOk = cpk.Load(filePath.c_str());
    if (!bOk)
        return -1;
    CPKFile* pCpkFile = cpk.Open("cbdata\\memoryLogFile.log");
    if (!pCpkFile)
        return -1;
    int origianlSize = pCpkFile->originalSize;
    cpk.Close(pCpkFile);
    char* lpBuf = new char[origianlSize + 1];
    memset(lpBuf, 0, sizeof(lpBuf));
    bOk = cpk.LoadFile(lpBuf, "cbdata\\memoryLogFile.log");
    lpBuf[origianlSize] = '\0';
    int len = strlen(lpBuf);
    delete[] lpBuf;
    return 0;
}