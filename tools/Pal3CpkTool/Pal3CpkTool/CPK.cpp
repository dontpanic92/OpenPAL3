#include "CPK.h"

#include <cassert>
#include <io.h>
#include "minilzo/minilzo.h"


static bool g_bCrcTableInitialized = false;
DWORD *CPK::CrcTable[256] = { 0 };
void* CPK::lzo_wrkmem = nullptr;



CPK::CPK()
{
    memset(this, 0, sizeof(CPK));
    this->m_eMode = CPKM_FileMapping;
    dwAllocationGranularity = GetAllocationGranularity();

    if (!g_bCrcTableInitialized) {
        InitCrcTable();
        lzo_wrkmem = new char[LZO1X_MEM_COMPRESS];
        memset(lzo_wrkmem, 0, LZO1X_MEM_COMPRESS);
    }

    gbVFile **ppVFile = m_pgbVFile;
    int iCount = 8;
    for (int i = 0; i < ARRAYSIZE(m_pgbVFile); i++) {
        gbVFile *pVFile = new gbVFile();
        if (pVFile) {
            pVFile->cpkFile.bValid = false;
            pVFile->cpkFile.lpMem = 0;
        } else {
            pVFile = 0;
        }
        m_pgbVFile[i] = pVFile;
    }
}

CPK::~CPK()
{
    for (int i = 0; i < ARRAYSIZE(m_pgbVFile); i++) {
        delete m_pgbVFile[i];
        m_pgbVFile[i] = nullptr;
    }
}

void CPK::InitCrcTable()
{
    int index; // edx
    DWORD **_crcTable; // ecx
    signed int iThunk; // esi
    int crcVal; // eax

    if (!g_bCrcTableInitialized) {
        index = 0;
        _crcTable = CPK::CrcTable;
        do {
            iThunk = 8;
            crcVal = index << 24;
            do {
                if (crcVal >= 0)
                    crcVal *= 2;
                else
                    crcVal = 2 * crcVal ^ 0x4C11DB7;
                --iThunk;
            } while (iThunk);
            *_crcTable = (DWORD *)crcVal;
            ++_crcTable;
            ++index;
        } while ((signed int)_crcTable < (signed int)&g_bCrcTableInitialized);
        g_bCrcTableInitialized = 1;
    }
}
DWORD CPK::GetAllocationGranularity(void)
{
    _SYSTEM_INFO SystemInfo;
    GetSystemInfo(&SystemInfo);
    return SystemInfo.dwAllocationGranularity;
}
#define IDA_LOBYTE(x)   (*((unsigned char*)&(x)))   // low byte
#define IDA_HIBYTE(x)   (*((unsigned char*)&(x)+1))
DWORD CPK::Crc(const char *name)
{
    const char *v2; // ecx
    int v3; // esi
    unsigned __int16 v4; // dx
    unsigned __int8 v5; // dl
    unsigned int i; // eax

    if (!*name)
        return 0;
    v2 = name + 1;
    v3 = *(unsigned __int8 *)name << 24;
    if (name[1]) {
        v3 = (*((unsigned __int8 *)name + 1) << 16) | (*(unsigned __int8 *)name << 24);
        v2 = name + 2;
        if (name[2]) {
            IDA_LOBYTE(v4) = 0;
            IDA_HIBYTE(v4) = name[2];
            v3 |= v4;
            v2 = name + 3;
            if (name[3]) {
                v3 |= *((unsigned __int8 *)name + 3);
                v2 = name + 4;
            }
        }
    }
    v5 = *v2;
    for (i = ~v3; v5; ++v2) {
        i = (unsigned int)CPK::CrcTable[i >> 24] ^ (v5 | (i << 8));
        v5 = v2[1];
    }
    unsigned int ret = ~i;
    return ret;
}

bool CPK::Close(CPKFile * pCpkFile)
{
    if (this->m_eMode == CPKM_FileMapping) {
        if (!pCpkFile->lpMapAddress) {
            //showMsgBox(0x10u, aErrorCeInvalid, aDProjectGbengi, 523);
            return false;
        }
        if (UnmapViewOfFile(pCpkFile->lpMapAddress) != TRUE) {
            //showMsgBox(0x10u, aErrorCeCannotU, aDProjectGbengi, 530);
            pCpkFile->bValid = false;
            return 0;
        }
        if (pCpkFile->bCompressed && pCpkFile->dwFileSize) {
            /*if (!(byte_10167011 & 1)) {
                byte_10167011 |= 1u;
                sub_1002DCF0(bufferHandle, 2, 1);
                atexit(unknown_libname_2);
            }*/
            //sub_1002E090((HANDLE *)bufferHandle, pCpkFile->pDest, pCpkFile->originalSize);

            //直接释放内存
            delete[] pCpkFile->lpMem;
            pCpkFile->lpMem = nullptr;
            pCpkFile->dwFileSize = 0;
        }
    }
    pCpkFile->bValid = false;
    --m_nOpenedFileNum;
    return 1;
}

bool CPK::IsFileExist(const char *lpString2)
{
    int nCurrent = GetTableIndex(lpString2);
    return nCurrent != -1;
}

int CPK::GetTableIndex(const char* lpString2)
{
    if (!m_bLoaded)
        return -1;
    int nCurrent = -1;
    CHAR String1[MAX_PATH] = { 0 };
    lstrcpyA(String1, lpString2);
    _strlwr_s(String1, sizeof(String1));
    unsigned int targetCRC = Crc(String1);
    nCurrent = GetTableIndexFromCRC(targetCRC);
    return nCurrent;
}


int CPK::GetTableIndexFromCRC(DWORD targetCRC)
{
    int nCurrent = -1;
    int nStart = 0;

    int dwValidTableNum = cpkHeader.dwValidTableNum;
    if (!dwValidTableNum)
        return nCurrent;

    while (true) {
        nCurrent = nStart + (dwValidTableNum - nStart) / 2;
        unsigned int vCRC = entries[nCurrent].dwCRC;
        if (targetCRC == vCRC) {
            int dwFlag = entries[nCurrent].dwFlag;
            if (dwFlag & CPKTableFlag_IsFile) {
                if (!(dwFlag & CPKTableFlag_IsDeleted))
                    break;
            }
        }
        if (dwValidTableNum == nStart + 1)
            return -1;
        if (targetCRC < vCRC)
            dwValidTableNum = nStart + (dwValidTableNum - nStart) / 2;
        else
            nStart += (dwValidTableNum - nStart) / 2;
        if (dwValidTableNum == nStart)
            return -1;
    }
    return nCurrent;
}

bool CPK::IsLoaded()
{
    return m_bLoaded;
}

HANDLE CPK::GetCPKHandle()
{
    return m_dwCPKHandle;
}

DWORD CPK::GetSize(CPKFile *pCpkFile)
{
    return pCpkFile->dwFileSize;
}

DWORD CPK::LoadFile(void *lpBuffer, const char *lpString2)
{
    int currIndex = GetTableIndex(lpString2);
    if (currIndex == -1)
        return 0;

    CPKTable* pFileEntry = &entries[currIndex];
    DWORD alignedOffset = pFileEntry->dwStartPos;
    DWORD dwFileOffsetLow = pFileEntry->dwStartPos;
    if (m_eMode == CPKM_FileMapping) {
        alignedOffset -= alignedOffset % dwAllocationGranularity;
        dwFileOffsetLow = alignedOffset;
    }
    int unalignedLen = pFileEntry->dwStartPos - alignedOffset;
    size_t mappedSize = unalignedLen + pFileEntry->dwPackedSize + pFileEntry->dwOriginSize;
    void* lpMapped;
    lpMapped = MapViewOfFile(m_dwCPKMappingHandle, FILE_MAP_READ, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
    if (!lpMapped) {
        return 0;
    }

    CpkZipUnzipParam param;
    param.flag = pFileEntry->dwFlag >> 0x10;
    param.srcSizeUnused = pFileEntry->dwPackedSize;
    param.srcSize = pFileEntry->dwPackedSize;
    param.bCompress = false;
    param.bResult = false;
    param.destSize = pFileEntry->dwOriginSize;
    param.destResultSize = pFileEntry->dwOriginSize;
    param.src = &((char*)lpMapped)[unalignedLen];
    param.dest = lpBuffer;

    executeZipUnZip(&param);
    UnmapViewOfFile(lpMapped);

    return param.bResult;
}

DWORD CPK::Seek(CPKFile *pCpkFile, int seekPos, ECPKSeekFileType seekType)
{
    int newPos; // eax

    switch (seekType) {
    case ECPKSeekFileType_Set: {
        newPos = seekPos;
    }break;
    case ECPKSeekFileType_Add: {
        newPos = pCpkFile->dwPointer + seekPos;
    }break;
    case ECPKSeekFileType_Sub: {
        newPos = pCpkFile->dwFileSize - seekPos;
    }break;
    default:
        return -1;
        break;
    }
    pCpkFile->dwPointer = newPos;
    if (this->m_eMode != CPKM_FileMapping)
        SetFilePointer(m_dwCPKHandle, pCpkFile->dwPointer + pCpkFile->pRecordEntry->dwStartPos, 0, 0);
    return pCpkFile->dwPointer;
}

DWORD CPK::Tell(CPKFile * pCpkFile)
{
    return pCpkFile->dwPointer;
}

void CPK::Rewind(CPKFile *pCpkFile)
{
    pCpkFile->dwPointer = 0;
}

void CPK::Reset()
{
    memset(&this->cpkHeader, 0, sizeof(this->cpkHeader));
    memset(this->entries, 0, sizeof(this->entries));
    this->m_dwCPKHandle = INVALID_HANDLE_VALUE;
    this->m_dwCPKMappingHandle = INVALID_HANDLE_VALUE;
    this->m_bLoaded = 0;
    memset(this->fileName, 0, sizeof(this->fileName));
    this->m_nOpenedFileNum = 0;
    for (int i = 0; i < ARRAYSIZE(m_pgbVFile); i++) {
        delete m_pgbVFile[i];
        m_pgbVFile[i] = nullptr;
    }
}

bool CPK::ReadFileEntryName(const CPKTable* pCpkFileEntry, char* lpBuffer, DWORD bufferLen)
{
    if (!m_bLoaded)
        return false;
    if (m_eMode != CPKM_FileMapping)
        return false;

    DWORD validOffset = pCpkFileEntry->dwStartPos + pCpkFileEntry->dwPackedSize;
    //DWORD dwFileOffsetLow = pCpkFileEntry->Offset + pCpkFileEntry->CompressedSize;
    DWORD alignOffset = validOffset % dwAllocationGranularity;

    if (m_eMode == CPKM_FileMapping) {
        validOffset -= alignOffset;
        //dwFileOffsetLow = alignedOffset;
    }

    LPVOID pInfoRecordBuf = MapViewOfFile(m_dwCPKMappingHandle, FILE_MAP_READ, 0, validOffset, pCpkFileEntry->dwExtraInfoSize + alignOffset);
    if (!pInfoRecordBuf) {
        DWORD dwLastErr = GetLastError();
        return false;
    }
    int unalignedLen = pCpkFileEntry->dwStartPos + pCpkFileEntry->dwPackedSize - validOffset;
    strcpy_s(lpBuffer, bufferLen, &((char*)pInfoRecordBuf)[alignOffset]);
    UnmapViewOfFile(pInfoRecordBuf);
    return true;
}

void CPK::SetOpenMode(ECPKMode openMode)
{
    if (!m_bLoaded) {
        m_eMode = openMode;
    }
}

bool CPK::BuildDirectoryTree(CPKDirectoryEntry& entry)
{
    if (!m_bLoaded)
        return false;

    //set up root directory info
    entry.vCRC = 0;
    entry.vParentCRC = 0;
    entry.iAttrib = CPKTableFlag_None;
    strcpy_s(entry.lpszName, sizeof(entry.lpszName), fileName);
    //用来记录所有已经处理过的节点，避免重复处理
    std::map<DWORD, CPKDirectoryEntry*> handledEntries;
    handledEntries.emplace(0, &entry);
    //遍历所有的节点，构造树状结构
    for (int i = 0; i < ARRAYSIZE(entries); i++) {
        //skip deleted files
        CPKTable& currFileEntry = entries[i];

        if (currFileEntry.dwFlag & CPKTableFlag_IsDeleted)
            continue;
        if (handledEntries.find(currFileEntry.dwCRC) != handledEntries.end()) {
            continue;
        }

        //find self parent
        if (!buildParent(currFileEntry, handledEntries)) {
            assert(false);
        }
    }
    return true;
}

bool CPK::buildParent(CPKTable& currFileEntry, std::map<DWORD, CPKDirectoryEntry*>& handledEntries)
{
    //当前节点已处理过，反回成功
    if (handledEntries.find(currFileEntry.dwCRC) != handledEntries.end())
        return true;
    //构造节点
    CPKDirectoryEntry* child = new CPKDirectoryEntry();
    child->iAttrib = currFileEntry.dwFlag;
    child->vCRC = currFileEntry.dwCRC;
    child->vParentCRC = currFileEntry.dwFatherCRC;
    ReadFileEntryName(&currFileEntry, child->lpszName, sizeof(child->lpszName));
    //如果当前节点的parent存在，则添加节点，退出递归
    if (handledEntries.find(child->vParentCRC) != handledEntries.end()) {
        //update child file name
        if (child->vParentCRC) {
            CHAR newName[MAX_PATH] = { 0 };
            sprintf_s(newName, "%s\\%s", handledEntries[child->vParentCRC]->lpszName, child->lpszName);
            strcpy_s(child->lpszName, sizeof(child->lpszName), newName);
        }
        handledEntries[child->vParentCRC]->childs.push_back(child);
        handledEntries[child->vCRC] = child;
        return true;
    }
    //get parent node
    int iIndex = GetTableIndexFromCRC(child->vParentCRC);
    if (iIndex == -1) {
        return false;
    }
    //build the parent
    if (buildParent(entries[iIndex], handledEntries)) {
        if (handledEntries.find(child->vParentCRC) != handledEntries.end()) {
            //update child file name
            if (child->vParentCRC) {
                CHAR newName[MAX_PATH] = { 0 };
                sprintf_s(newName, "%s\\%s", handledEntries[child->vParentCRC]->lpszName, child->lpszName);
                strcpy_s(child->lpszName, sizeof(child->lpszName), newName);
            }
            handledEntries[child->vParentCRC]->childs.push_back(child);
            handledEntries[child->vCRC] = child;
            return true;
        }
    }
    return false;
}

bool CPK::Load(const char *lpFileName)
{
    strcpy_s(fileName, sizeof(fileName), lpFileName);
    if (m_bLoaded) {
        //showMsgBox(0x10u, aErrorCeCpkAlre, aDProjectGbengi, 161);
        if (m_eMode == CPKM_FileMapping)
            CloseHandle(m_dwCPKMappingHandle);
        CloseHandle(m_dwCPKHandle);
        Reset();
    }
    m_dwCPKHandle = CreateFileA(lpFileName, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_SUPPORTS_OPEN_BY_FILE_ID | FILE_ATTRIBUTE_NORMAL, NULL);
    if (m_dwCPKHandle == INVALID_HANDLE_VALUE) {
        /*showMsgBox(0x10u, aErrorCeCannotO, aDProjectGbengi, 175);
        showMessageBox(aCouldnTOpenPac, lpFileName);*/
        CloseHandle(m_dwCPKHandle);
        return 0;
    }
    DWORD NumberOfBytesRead;
    ReadFile(m_dwCPKHandle, &cpkHeader, sizeof(CPKHeader), &NumberOfBytesRead, 0);// 读文件头
    if (cpkHeader.dwLable != 0x1A545352)// 验证文件头签名
    {
        //showMsgBox(0x10u, aErrorCeUnknowC, aDProjectGbengi, 185);
        //showMessageBox(aUnknowFileForm, lpFileName);
        CloseHandle(m_dwCPKHandle);
        return 0;
    }
    int totalRead = sizeof(CPKTable) * cpkHeader.dwMaxFileNum;
    if (!ReadFile(m_dwCPKHandle, entries, totalRead, &NumberOfBytesRead, 0) ||
        NumberOfBytesRead != totalRead) {
        //showMsgBox(0x10u, aErrorCeCannotL, aDProjectGbengi, 200);
        //showMessageBox(aCouldNotLoadTa, lpFileName);
        CloseHandle(m_dwCPKHandle);
        return 0;
    }
    if (m_eMode != CPKM_FileMapping) {
        m_bLoaded = 1;
        return true;
    }
    m_dwCPKMappingHandle = CreateFileMappingA(m_dwCPKHandle, 0, 2u, 0, 0, 0);
    if (!m_dwCPKMappingHandle) {
        //showMsgBox(0x10u, aErrorCeCannotC, aDProjectGbengi, 215);
        //showMessageBox(aCouldnTCreateF, lpFileName);
        CloseHandle(m_dwCPKHandle);
        return 0;
    }
    if (GetLastError() != ERROR_ALREADY_EXISTS) {
        m_bLoaded = true;
        return true;
    } else {
        //showMsgBox(0x10u, aErrorCeMapping, aDProjectGbengi, 225);
        CloseHandle(m_dwCPKMappingHandle);
        m_dwCPKMappingHandle = 0;
        //showMessageBox(aFileMappingSHa, lpFileName);
        CloseHandle(m_dwCPKHandle);
        return false;
    }
}

bool CPK::Read(void* lpBuffer, DWORD nNumberOfBytesToRead, CPKFile *pCpkFile)
{
    if (m_eMode == CPKM_FileMapping)
        memcpy(lpBuffer, (char *)pCpkFile->lpMem + pCpkFile->dwPointer, nNumberOfBytesToRead);
    else {
        DWORD NumberOfBytesRead;
        BOOL bSucc = ReadFile(this->m_dwCPKHandle, lpBuffer, nNumberOfBytesToRead, &NumberOfBytesRead, 0);
        bSucc &= NumberOfBytesRead == nNumberOfBytesToRead;
    }
    pCpkFile->dwPointer += nNumberOfBytesToRead;
    return true;
}

bool CPK::Unload()
{
    if (!m_bLoaded)
        return 0;
    if (m_eMode == CPKM_FileMapping)
        CloseHandle(m_dwCPKMappingHandle);
    CloseHandle(m_dwCPKHandle);
    memset(&cpkHeader, 0, sizeof(cpkHeader));
    memset(entries, 0, sizeof(entries));
    m_dwCPKHandle = INVALID_HANDLE_VALUE;
    m_dwCPKMappingHandle = INVALID_HANDLE_VALUE;
    m_bLoaded = false;
    memset(fileName, 0, sizeof(fileName));
    m_nOpenedFileNum = 0;
    for (int i = 0; i < ARRAYSIZE(m_pgbVFile); i++) {
        m_pgbVFile[i]->cpkFile.bValid = false;
    }
    return true;
}

char* CPK::ReadLine(char *lpBuffer, int ReadSize, CPKFile *pCpkFile)
{
    if (ReadSize - 1 <= 0)
        return 0;

    int i = 0;
    for (; i < ReadSize - 1; i++, pCpkFile->dwPointer++) {
        if (pCpkFile->dwPointer >= pCpkFile->dwFileSize)
            break;
        lpBuffer[i] = ((char *)pCpkFile->lpMem)[pCpkFile->dwPointer];
        if (lpBuffer[i] == '\n' && i >= 1 && lpBuffer[i - 1] == '\r')
            break;
    }

    if (i <= 0)
        return nullptr;
    lpBuffer[i] = '\0';
    return lpBuffer;
}

bool CPK::IsValidCPK(const char *lpFileName)
{

    bool result = _access(lpFileName, 0) != -1;
    if (result) {
        HANDLE hFile = CreateFileA(lpFileName, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0x10000080u, 0);
        if (hFile == INVALID_HANDLE_VALUE) {
            //showMessageBox(aCouldnTOpenFil, lpFileName);
            return false;
        } else {
            CPKHeader cpkHeader;
            memset(&cpkHeader, 0, sizeof(cpkHeader));
            DWORD NumberOfBytesRead;
            ReadFile(hFile, &cpkHeader, sizeof(CPKHeader), &NumberOfBytesRead, NULL);
            CloseHandle(hFile);
            if (cpkHeader.dwLable == 0x1A545352) {
                if (cpkHeader.dwVersion == 1) {
                    result = 1;
                } else {
                    //showMessageBox(aWrongFileVersi, cpkHeader.dwCheckFlag);
                    return false;
                }
            } else {
                //showMessageBox(aUnknowFileForm, lpFileName);
                return false;
            }
        }
    }
    return result;
}

CPKFile* CPK::Open(const char *lpString2)
{
    if (!m_bLoaded)
        return nullptr;
    int currIndex = GetTableIndex(lpString2);
    if (currIndex == -1)
        return nullptr;

    gbVFile* pVFile = OpenTableIndex(currIndex);
    if (!pVFile)
        return nullptr;
    strcpy_s(pVFile->fileName, sizeof(pVFile->fileName), lpString2);
    return &pVFile->cpkFile;
}

CPKFile * CPK::Open(DWORD vCRC, const char* saveFileName)
{
    if (!m_bLoaded)
        return nullptr;
    int currIndex = GetTableIndexFromCRC(vCRC);
    if (currIndex == -1)
        return nullptr;

    gbVFile* pVFile = OpenTableIndex(currIndex);
    if (!pVFile)
        return nullptr;
    strcpy_s(pVFile->fileName, sizeof(pVFile->fileName), saveFileName);
    return &pVFile->cpkFile;
}

char CPK::ReadChar(CPKFile *pCpkFile)
{
    char result; // al

    if (pCpkFile->dwPointer >= pCpkFile->dwFileSize)
        return -1;
    result = ((char*)pCpkFile->lpMem)[pCpkFile->dwPointer++];
    return result;
}

DWORD CPK::Compress(void *dest, void *src, unsigned int srcSize)
{
    CpkZipUnzipParam param; // [esp+0h] [ebp-24h]

    param.flag = 2;
    param.bCompress = true;
    param.src = src;
    param.srcSize = srcSize;
    param.srcSizeUnused = srcSize;
    param.dest = dest;
    param.destSize = 0;
    executeZipUnZip(&param);
    return param.destResultSize;
}

DWORD CPK::DeCompress(void* dest, void* src, DWORD srcSize)
{
    CpkZipUnzipParam param; // [esp+0h] [ebp-24h]

    param.flag = 2;
    param.bCompress = false;
    param.src = src;
    param.srcSize = srcSize;
    param.srcSizeUnused = srcSize;
    param.destSize = srcSize * 2;
    param.dest = dest;
    param.bResult = 0;
    param.destResultSize = 0;
    executeZipUnZip(&param);
    return param.destResultSize;
}

int CPK::executeZipUnZip(CpkZipUnzipParam *param)
{
    int result; // eax

    result = param->flag - 1;
    if (param->flag == 1) {
        param->destResultSize = param->srcSize;
    } else {
        result = param->flag - 2;
        if (param->flag != 2)
            return result;
        if (param->bCompress) {
            result = lzo1x_1_compress((unsigned __int8 *)param->src,
                param->srcSize,
                (BYTE*)param->dest,
                (unsigned int*)&param->destResultSize,
                (void*)lzo_wrkmem);
            if (result) {
                param->bResult = 0;
                return -1;
            }
        } else {
            result = lzo1x_decompress(
                (unsigned __int8 *)param->src,
                param->srcSize,
                (BYTE *)param->dest,
                (unsigned int*)&param->destResultSize,
                nullptr
            );
            if (result != LZO_E_OK) {
                param->bResult = 0;
                return -1;
            }
        }
    }
    param->bResult = 1;
    return result;
}

gbVFile* CPK::OpenTableIndex(int iFileTableIndex)
{
    if ((signed int)m_nOpenedFileNum >= ARRAYSIZE(m_pgbVFile) - 1) {
        //showMsgBox(0x10u, aErrorCeCannotO_0, aDProjectGbengi, 361);
        return nullptr;
    }

    CPKTable* pFileEntry = &entries[iFileTableIndex];
    DWORD alignedOffset = pFileEntry->dwStartPos;
    DWORD dwFileOffsetLow = pFileEntry->dwStartPos;
    if (m_eMode == CPKM_FileMapping) {
        alignedOffset -= alignedOffset % dwAllocationGranularity;
        dwFileOffsetLow = alignedOffset;
    }
    int unalignedLen = pFileEntry->dwStartPos - alignedOffset;
    size_t mappedSize = unalignedLen + pFileEntry->dwPackedSize;
    int iIndex = 0;
    for (; iIndex < ARRAYSIZE(m_pgbVFile); iIndex++) {
        if (!m_pgbVFile[iIndex]->cpkFile.bValid)
            break;
    }
    if (iIndex >= 8) {
        return 0;
    }
    gbVFile* pVFile = m_pgbVFile[iIndex];
    if (!pVFile) {
        return 0;
    }
    CPKFile* pCpkFile = &pVFile->cpkFile;

    //open the cpk file and decompress data
    pCpkFile->bValid = true;
    //strcpy_s(pVFile->fileName, sizeof(pVFile->fileName), lpString2);
    pCpkFile->lpMapAddress = 0;
    if (m_eMode == CPKM_FileMapping) {
        void* lpMapped = MapViewOfFile(m_dwCPKMappingHandle, 4u, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
        if (!lpMapped) {
            DWORD dwErr = GetLastError();
            OutputDebugStringA("error");
        }
        pCpkFile->lpMapAddress = lpMapped;
        if (!lpMapped) {
            pCpkFile->bValid = false;
            return nullptr;
        }
    }
    pCpkFile->dwCRC = pFileEntry->dwCRC;
    pCpkFile->nTableIndex = iFileTableIndex;
    pCpkFile->dwFatherCRC = pFileEntry->dwFatherCRC;
    pCpkFile->pRecordEntry = pFileEntry;// 记录entry结构指针
    pCpkFile->lpStartAddress = &(((char *)pCpkFile->lpMapAddress)[unalignedLen]);

    pCpkFile->bCompressed = (pFileEntry->dwFlag & 0xFFFF0000) != 0x10000;
    DWORD originalSize = pFileEntry->dwOriginSize;
    pCpkFile->dwOffset = unalignedLen;
    pCpkFile->dwFileSize = originalSize;
    pCpkFile->dwPointer = 0;
    if (pCpkFile->bCompressed && originalSize) {
        if (m_eMode != CPKM_FileMapping) {
            pCpkFile->bValid = false;
            return nullptr;
        }
        /*if (!(byte_10167011 & 1)) {
            byte_10167011 |= 1u;
            sub_1002DCF0(bufferHandle, 2, 1);
            atexit(unknown_libname_2);
        }*/
        void* pDest = new char[pCpkFile->dwFileSize];
        //v27 = cpkAllocBuffer((HANDLE *)bufferHandle, pCpkFile->compressedSize);
        pCpkFile->lpMem = pDest;
        if (!pDest) {
            //showMsgBox(0x10u, aErrorCeCannotA, aDProjectGbengi, 464);
            UnmapViewOfFile(pCpkFile->lpMapAddress);
            pCpkFile->bValid = false;
            return nullptr;
        }
        CpkZipUnzipParam param; // [esp+2Ch] [ebp-168h]
        param.flag = pFileEntry->dwFlag >> 0x10;
        param.srcSizeUnused = pFileEntry->dwPackedSize;
        param.srcSize = pFileEntry->dwPackedSize;
        param.bCompress = 0;
        param.bResult = 0;
        param.destSize = pFileEntry->dwOriginSize;
        param.destResultSize = pFileEntry->dwOriginSize;
        param.src = pCpkFile->lpStartAddress;
        param.dest = pCpkFile->lpMem;
        executeZipUnZip(&param);
        if (!param.bResult) {
            //showMsgBox(0x10u, aErrorCeCannotD, aDProjectGbengi, 486);
            //cpkUnmapViewOfFile(pVFile->lpMapFileBase);
            /*if (!(byte_10167011 & 1)) {
                byte_10167011 |= 1u;
                sub_1002DCF0(bufferHandle, 2, 1);
                atexit(unknown_libname_2);
            }*/
            //sub_1002E090((HANDLE *)bufferHandle, pCpkFile->pDest, pCpkFile->originalSize);
            delete[] pCpkFile->lpMem;
            pCpkFile->bValid = false;
            return nullptr;
        }
    } else {
        pCpkFile->lpMem = pCpkFile->lpStartAddress;
    }
    if (m_eMode != CPKM_FileMapping)
        SetFilePointer(m_dwCPKHandle, pCpkFile->pRecordEntry->dwStartPos, 0, 0);
    ++m_nOpenedFileNum;
    //return &pVFile->cpkFile;
    return pVFile;
}

bool CPK::GetFileSize(DWORD &CompressedSize, DWORD &OriginalSize, DWORD targetCRC)
{
    if (!targetCRC) {
        CompressedSize = targetCRC;
        OriginalSize = targetCRC;
        return true;
    }

    int iIndex = GetTableIndexFromCRC(targetCRC);
    if (iIndex == -1)
        return false;
    CompressedSize = entries[iIndex].dwPackedSize;
    OriginalSize = entries[iIndex].dwOriginSize;
    return true;
}

bool CPK::IsDir(DWORD dwTargetCRC)
{
    int iIndex = GetTableIndexFromCRC(dwTargetCRC);
    if (iIndex == -1)
        return false;
    return entries[iIndex].dwFlag & CPKTableFlag_IsDir;
}
