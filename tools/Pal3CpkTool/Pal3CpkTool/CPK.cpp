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
    this->dwOpenMode = ECPKMode_Mapped;
    dwAllocationGranularity = GetAllocationGranularity();

    if (!g_bCrcTableInitialized) {
        InitCrcTable();
        lzo_wrkmem = new char[LZO1X_MEM_COMPRESS];
        memset(lzo_wrkmem, 0, LZO1X_MEM_COMPRESS);
    }

    gbVFile **ppVFile = vFiles;
    int iCount = 8;
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        gbVFile *pVFile = new gbVFile();
        if (pVFile) {
            pVFile->cpkFile.bOpened = false;
            pVFile->cpkFile.pDest = 0;
        } else {
            pVFile = 0;
        }
        vFiles[i] = pVFile;
    }
}

CPK::~CPK()
{
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        delete vFiles[i];
        vFiles[i] = nullptr;
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
    if (this->dwOpenMode == ECPKMode_Mapped) {
        if (!pCpkFile->lpMapFileBase) {
            //showMsgBox(0x10u, aErrorCeInvalid, aDProjectGbengi, 523);
            return false;
        }
        if (UnmapViewOfFile(pCpkFile->lpMapFileBase) != TRUE) {
            //showMsgBox(0x10u, aErrorCeCannotU, aDProjectGbengi, 530);
            pCpkFile->bOpened = false;
            return 0;
        }
        if (pCpkFile->isCompressed && pCpkFile->originalSize) {
            /*if (!(byte_10167011 & 1)) {
                byte_10167011 |= 1u;
                sub_1002DCF0(bufferHandle, 2, 1);
                atexit(unknown_libname_2);
            }*/
            //sub_1002E090((HANDLE *)bufferHandle, pCpkFile->pDest, pCpkFile->originalSize);

            //直接释放内存
            delete[] pCpkFile->pDest;
            pCpkFile->pDest = nullptr;
            pCpkFile->originalSize = 0;
        }
    }
    pCpkFile->bOpened = false;
    --dwVFileOpenedCount;
    return 1;
}

bool CPK::IsFileExist(const char *lpString2)
{
    int nCurrent = GetTableIndex(lpString2);
    return nCurrent != -1;
}

int CPK::GetTableIndex(const char* lpString2)
{
    if (!isLoaded)
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

    int dwEntryCount = cpkHeader.dwCount;
    if (!dwEntryCount)
        return nCurrent;

    while (true) {
        nCurrent = nStart + (dwEntryCount - nStart) / 2;
        unsigned int vCRC = entries[nCurrent].vCRC;
        if (targetCRC == vCRC) {
            int nAttrib = entries[nCurrent].Attrib;
            if (nAttrib & CpkFileAttrib_IsFile) {
                if (!(nAttrib & CpkFileAttrib_IsDeleted))
                    break;
            }
        }
        if (dwEntryCount == nStart + 1)
            return -1;
        if (targetCRC < vCRC)
            dwEntryCount = nStart + (dwEntryCount - nStart) / 2;
        else
            nStart += (dwEntryCount - nStart) / 2;
        if (dwEntryCount == nStart)
            return -1;
    }
    return nCurrent;
}

bool CPK::IsLoaded()
{
    return isLoaded;
}

HANDLE CPK::GetCPKHandle()
{
    return fileHandle;
}

DWORD CPK::GetSize(CPKFile *pCpkFile)
{
    return pCpkFile->originalSize;
}

DWORD CPK::LoadFile(void *lpBuffer, const char *lpString2)
{
    int currIndex = GetTableIndex(lpString2);
    if (currIndex == -1)
        return 0;

    CpkFileEntry* pFileEntry = &entries[currIndex];
    DWORD alignedOffset = pFileEntry->Offset;
    DWORD dwFileOffsetLow = pFileEntry->Offset;
    if (dwOpenMode == ECPKMode_Mapped) {
        alignedOffset -= alignedOffset % dwAllocationGranularity;
        dwFileOffsetLow = alignedOffset;
    }
    int unalignedLen = pFileEntry->Offset - alignedOffset;
    size_t mappedSize = unalignedLen + pFileEntry->CompressedSize + pFileEntry->OriginalSize;
    void* lpMapped;
    lpMapped = MapViewOfFile(fileMappingHandle, FILE_MAP_READ, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
    if (!lpMapped) {
        return 0;
    }

    CpkZipUnzipParam param;
    param.flag = pFileEntry->Attrib >> 0x10;
    param.srcSizeUnused = pFileEntry->CompressedSize;
    param.srcSize = pFileEntry->CompressedSize;
    param.bCompress = false;
    param.bResult = false;
    param.destSize = pFileEntry->OriginalSize;
    param.destResultSize = pFileEntry->OriginalSize;
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
        newPos = pCpkFile->fileOffset + seekPos;
    }break;
    case ECPKSeekFileType_Sub: {
        newPos = pCpkFile->originalSize - seekPos;
    }break;
    default:
        return -1;
        break;
    }
    pCpkFile->fileOffset = newPos;
    if (this->dwOpenMode != ECPKMode_Mapped)
        SetFilePointer(fileHandle, pCpkFile->fileOffset + pCpkFile->pRecordEntry->Offset, 0, 0);
    return pCpkFile->fileOffset;
}

DWORD CPK::Tell(CPKFile * pCpkFile)
{
    return pCpkFile->fileOffset;
}

void CPK::Rewind(CPKFile *pCpkFile)
{
    pCpkFile->fileOffset = 0;
}

void CPK::Reset()
{
    memset(&this->cpkHeader, 0, sizeof(this->cpkHeader));
    memset(this->entries, 0, sizeof(this->entries));
    this->fileHandle = (HANDLE)-1;
    this->fileMappingHandle = (HANDLE)-1;
    this->isLoaded = 0;
    memset(this->fileName, 0, sizeof(this->fileName));
    this->dwVFileOpenedCount = 0;
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        delete vFiles[i];
        vFiles[i] = nullptr;
    }
}

bool CPK::ReadFileEntryName(const CpkFileEntry* pCpkFileEntry, char* lpBuffer, DWORD bufferLen)
{
    if (!isLoaded)
        return false;
    if (dwOpenMode != ECPKMode_Mapped)
        return false;

    DWORD validOffset = pCpkFileEntry->Offset + pCpkFileEntry->CompressedSize;
    //DWORD dwFileOffsetLow = pCpkFileEntry->Offset + pCpkFileEntry->CompressedSize;
    DWORD alignOffset = validOffset % dwAllocationGranularity;

    if (dwOpenMode == ECPKMode_Mapped) {
        validOffset -= alignOffset;
        //dwFileOffsetLow = alignedOffset;
    }

    LPVOID pInfoRecordBuf = MapViewOfFile(fileMappingHandle, FILE_MAP_READ, 0, validOffset, pCpkFileEntry->InfoRecordSize + alignOffset);
    if (!pInfoRecordBuf) {
        DWORD dwLastErr = GetLastError();
        return false;
    }
    int unalignedLen = pCpkFileEntry->Offset + pCpkFileEntry->CompressedSize - validOffset;
    strcpy_s(lpBuffer, bufferLen, &((char*)pInfoRecordBuf)[alignOffset]);
    UnmapViewOfFile(pInfoRecordBuf);
    return true;
}

void CPK::SetOpenMode(ECPKMode openMode)
{
    if (!isLoaded) {
        dwOpenMode = openMode;
    }
}

bool CPK::buildDirectoryTree(CPKDirectoryEntry& entry)
{
    if (!isLoaded)
        return false;

    //set up root directory info
    entry.vCRC = 0;
    entry.vParentCRC = 0;
    entry.iAttrib = CpkFileAttrib_None;
    strcpy_s(entry.lpszName, sizeof(entry.lpszName), fileName);
    //用来记录所有已经处理过的节点，避免重复处理
    std::map<DWORD, CPKDirectoryEntry*> handledEntries;
    handledEntries.emplace(0, &entry);
    //遍历所有的节点，构造树状结构
    for (int i = 0; i < ARRAYSIZE(entries); i++) {
        //skip deleted files
        CpkFileEntry& currFileEntry = entries[i];

        if (currFileEntry.Attrib & CpkFileAttrib_IsDeleted)
            continue;
        if (handledEntries.find(currFileEntry.vCRC) != handledEntries.end()) {
            continue;
        }

        //find self parent
        if (!buildParent(currFileEntry, handledEntries)) {
            assert(false);
        }
    }
    return true;
}

bool CPK::buildParent(CpkFileEntry& currFileEntry, std::map<DWORD, CPKDirectoryEntry*>& handledEntries)
{
    //当前节点已处理过，反回成功
    if (handledEntries.find(currFileEntry.vCRC) != handledEntries.end())
        return true;
    //构造节点
    CPKDirectoryEntry* child = new CPKDirectoryEntry();
    child->iAttrib = currFileEntry.Attrib;
    child->vCRC = currFileEntry.vCRC;
    child->vParentCRC = currFileEntry.vParentCRC;
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
    if (isLoaded) {
        //showMsgBox(0x10u, aErrorCeCpkAlre, aDProjectGbengi, 161);
        if (dwOpenMode == ECPKMode_Mapped)
            CloseHandle(fileMappingHandle);
        CloseHandle(fileHandle);
        Reset();
    }
    fileHandle = CreateFileA(lpFileName, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_SUPPORTS_OPEN_BY_FILE_ID | FILE_ATTRIBUTE_NORMAL, NULL);
    if (fileHandle == (HANDLE)-1) {
        /*showMsgBox(0x10u, aErrorCeCannotO, aDProjectGbengi, 175);
        showMessageBox(aCouldnTOpenPac, lpFileName);*/
        CloseHandle(fileHandle);
        return 0;
    }
    DWORD NumberOfBytesRead;
    ReadFile(fileHandle, &cpkHeader, sizeof(CpkHeader), &NumberOfBytesRead, 0);// 读文件头
    if (cpkHeader.signature != 0x1A545352)// 验证文件头签名
    {
        //showMsgBox(0x10u, aErrorCeUnknowC, aDProjectGbengi, 185);
        //showMessageBox(aUnknowFileForm, lpFileName);
        CloseHandle(fileHandle);
        return 0;
    }
    int totalRead = sizeof(CpkFileEntry) * cpkHeader.entryCapacity;
    if (!ReadFile(fileHandle, entries, totalRead, &NumberOfBytesRead, 0) ||
        NumberOfBytesRead != totalRead) {
        //showMsgBox(0x10u, aErrorCeCannotL, aDProjectGbengi, 200);
        //showMessageBox(aCouldNotLoadTa, lpFileName);
        CloseHandle(fileHandle);
        return 0;
    }
    if (dwOpenMode != ECPKMode_Mapped) {
        isLoaded = 1;
        return true;
    }
    fileMappingHandle = CreateFileMappingA(fileHandle, 0, 2u, 0, 0, 0);
    if (!fileMappingHandle) {
        //showMsgBox(0x10u, aErrorCeCannotC, aDProjectGbengi, 215);
        //showMessageBox(aCouldnTCreateF, lpFileName);
        CloseHandle(fileHandle);
        return 0;
    }
    if (GetLastError() != ERROR_ALREADY_EXISTS) {
        isLoaded = true;
        return true;
    } else {
        //showMsgBox(0x10u, aErrorCeMapping, aDProjectGbengi, 225);
        CloseHandle(fileMappingHandle);
        fileMappingHandle = 0;
        //showMessageBox(aFileMappingSHa, lpFileName);
        CloseHandle(fileHandle);
        return false;
    }
}

bool CPK::Read(void* lpBuffer, DWORD nNumberOfBytesToRead, CPKFile *pCpkFile)
{
    if (dwOpenMode == ECPKMode_Mapped)
        memcpy(lpBuffer, (char *)pCpkFile->pDest + pCpkFile->fileOffset, nNumberOfBytesToRead);
    else {
        DWORD NumberOfBytesRead;
        BOOL bSucc = ReadFile(this->fileHandle, lpBuffer, nNumberOfBytesToRead, &NumberOfBytesRead, 0);
        bSucc &= NumberOfBytesRead == nNumberOfBytesToRead;
    }
    pCpkFile->fileOffset += nNumberOfBytesToRead;
    return true;
}

bool CPK::Unload()
{
    if (!isLoaded)
        return 0;
    if (dwOpenMode == ECPKMode_Mapped)
        CloseHandle(fileMappingHandle);
    CloseHandle(fileHandle);
    memset(&cpkHeader, 0, sizeof(cpkHeader));
    memset(entries, 0, sizeof(entries));
    fileHandle = INVALID_HANDLE_VALUE;
    fileMappingHandle = INVALID_HANDLE_VALUE;
    isLoaded = false;
    memset(fileName, 0, sizeof(fileName));
    dwVFileOpenedCount = 0;
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        vFiles[i]->cpkFile.bOpened = false;
    }
    return true;
}

char* CPK::ReadLine(char *lpBuffer, int ReadSize, CPKFile *pCpkFile)
{
    if (ReadSize - 1 <= 0)
        return 0;

    int i = 0;
    for (; i < ReadSize - 1; i++, pCpkFile->fileOffset++) {
        if (pCpkFile->fileOffset >= pCpkFile->originalSize)
            break;
        lpBuffer[i] = ((char *)pCpkFile->pDest)[pCpkFile->fileOffset];
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
        if (hFile == (HANDLE)-1) {
            //showMessageBox(aCouldnTOpenFil, lpFileName);
            return false;
        } else {
            CpkHeader cpkHeader;
            memset(&cpkHeader, 0, sizeof(cpkHeader));
            DWORD NumberOfBytesRead;
            ReadFile(hFile, &cpkHeader, sizeof(CpkHeader), &NumberOfBytesRead, NULL);
            CloseHandle(hFile);
            if (cpkHeader.signature == 0x1A545352) {
                if (cpkHeader.dwCheckFlag == 1) {
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
    if (!isLoaded)
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
    if (!isLoaded)
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

    if (pCpkFile->fileOffset >= pCpkFile->originalSize)
        return -1;
    result = ((char*)pCpkFile->pDest)[pCpkFile->fileOffset++];
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
    if ((signed int)dwVFileOpenedCount >= ARRAYSIZE(vFiles) - 1) {
        //showMsgBox(0x10u, aErrorCeCannotO_0, aDProjectGbengi, 361);
        return nullptr;
    }

    CpkFileEntry* pFileEntry = &entries[iFileTableIndex];
    DWORD alignedOffset = pFileEntry->Offset;
    DWORD dwFileOffsetLow = pFileEntry->Offset;
    if (dwOpenMode == ECPKMode_Mapped) {
        alignedOffset -= alignedOffset % dwAllocationGranularity;
        dwFileOffsetLow = alignedOffset;
    }
    int unalignedLen = pFileEntry->Offset - alignedOffset;
    size_t mappedSize = unalignedLen + pFileEntry->CompressedSize;
    int iIndex = 0;
    for (; iIndex < ARRAYSIZE(vFiles); iIndex++) {
        if (!vFiles[iIndex]->cpkFile.bOpened)
            break;
    }
    if (iIndex >= 8) {
        return 0;
    }
    gbVFile* pVFile = vFiles[iIndex];
    if (!pVFile) {
        return 0;
    }
    CPKFile* pCpkFile = &pVFile->cpkFile;

    //open the cpk file and decompress data
    pCpkFile->bOpened = 1;
    //strcpy_s(pVFile->fileName, sizeof(pVFile->fileName), lpString2);
    pCpkFile->lpMapFileBase = 0;
    if (dwOpenMode == ECPKMode_Mapped) {
        void* lpMapped = MapViewOfFile(fileMappingHandle, 4u, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
        if (!lpMapped) {
            DWORD dwErr = GetLastError();
            OutputDebugStringA("error");
        }
        pCpkFile->lpMapFileBase = lpMapped;
        if (!lpMapped) {
            pCpkFile->bOpened = 0;
            return nullptr;
        }
    }
    pCpkFile->vCRC = pFileEntry->vCRC;
    pCpkFile->fileIndex = iFileTableIndex;
    pCpkFile->vParentCRC = pFileEntry->vParentCRC;
    pCpkFile->pRecordEntry = pFileEntry;// 记录entry结构指针
    pCpkFile->pSrc = &(((char *)pCpkFile->lpMapFileBase)[unalignedLen]);

    pCpkFile->isCompressed = (pFileEntry->Attrib & 0xFFFF0000) != 0x10000;
    DWORD originalSize = pFileEntry->OriginalSize;
    pCpkFile->srcOffset = unalignedLen;
    pCpkFile->originalSize = originalSize;
    pCpkFile->fileOffset = 0;
    if (pCpkFile->isCompressed && originalSize) {
        if (dwOpenMode != ECPKMode_Mapped) {
            pCpkFile->bOpened = 0;
            return nullptr;
        }
        /*if (!(byte_10167011 & 1)) {
            byte_10167011 |= 1u;
            sub_1002DCF0(bufferHandle, 2, 1);
            atexit(unknown_libname_2);
        }*/
        void* pDest = new char[pCpkFile->originalSize];
        //v27 = cpkAllocBuffer((HANDLE *)bufferHandle, pCpkFile->compressedSize);
        pCpkFile->pDest = pDest;
        if (!pDest) {
            //showMsgBox(0x10u, aErrorCeCannotA, aDProjectGbengi, 464);
            UnmapViewOfFile(pCpkFile->lpMapFileBase);
            pCpkFile->bOpened = 0;
            return nullptr;
        }
        CpkZipUnzipParam param; // [esp+2Ch] [ebp-168h]
        param.flag = pFileEntry->Attrib >> 0x10;
        param.srcSizeUnused = pFileEntry->CompressedSize;
        param.srcSize = pFileEntry->CompressedSize;
        param.bCompress = 0;
        param.bResult = 0;
        param.destSize = pFileEntry->OriginalSize;
        param.destResultSize = pFileEntry->OriginalSize;
        param.src = pCpkFile->pSrc;
        param.dest = pCpkFile->pDest;
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
            delete[] pCpkFile->pDest;
            pCpkFile->bOpened = 0;
            return nullptr;
        }
    } else {
        pCpkFile->pDest = pCpkFile->pSrc;
    }
    if (dwOpenMode != ECPKMode_Mapped)
        SetFilePointer(fileHandle, pCpkFile->pRecordEntry->Offset, 0, 0);
    ++dwVFileOpenedCount;
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
    CompressedSize = entries[iIndex].CompressedSize;
    OriginalSize = entries[iIndex].OriginalSize;
    return true;
}

bool CPK::IsDir(DWORD dwTargetCRC)
{
    int iIndex = GetTableIndexFromCRC(dwTargetCRC);
    if (iIndex == -1)
        return false;
    return entries[iIndex].Attrib & CpkFileAttrib_IsDir;
}
