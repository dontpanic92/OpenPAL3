#include "CPK.h"

#include <cassert>
#include <io.h>


static bool g_bCrcTableInitialized = false;
DWORD *CPK::CrcTable[256] = { 0 };
static int encryptTable[256] = { 0 };



CPK::CPK()
{
    memset(this, 0, sizeof(CPK));
    this->dwOpenMode = ECPKMode_Mapped;
    dwAllocationGranularity = GetAllocationGranularity();

    if (!g_bCrcTableInitialized)
        InitCrcTable();

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
        if (pCpkFile->bFlag && pCpkFile->originalSize) {
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
    pCpkFile->bOpened = 0;
    --dwVFileOpened;
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
    lpMapped = MapViewOfFile(fileMappingHandle, 4u, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
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
    this->dwVFileOpened = 0;
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        delete vFiles[i];
        vFiles[i] = nullptr;
    }
}

void CPK::SetOpenMode(ECPKMode openMode)
{
    if (!isLoaded) {
        dwOpenMode = openMode;
    }
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
    fileHandle = CreateFileA(lpFileName, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0x10000080u, NULL);
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
        isLoaded = 1;
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
    fileHandle = (HANDLE)-1;
    fileMappingHandle = (HANDLE)-1;
    isLoaded = 0;
    memset(fileName, 0, sizeof(fileName));
    dwVFileOpened = 0;
    for (int i = 0; i < ARRAYSIZE(vFiles); i++) {
        vFiles[i]->cpkFile.bOpened = false;
    }
    return true;
}

char* CPK::ReadLine(char *lpBuffer, int ReadSize, struct CPKFile *pCpkFile)
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
    if ((signed int)dwVFileOpened >= ARRAYSIZE(vFiles) - 1) {
        //showMsgBox(0x10u, aErrorCeCannotO_0, aDProjectGbengi, 361);
        return nullptr;
    }
    int currIndex = GetTableIndex(lpString2);
    if (currIndex == -1)
        return nullptr;
    CpkFileEntry* pFileEntry = &entries[currIndex];
    DWORD alignedOffset = pFileEntry->Offset;
    DWORD dwFileOffsetLow = pFileEntry->Offset;
    if (dwOpenMode == ECPKMode_Mapped) {
        alignedOffset -= alignedOffset % dwAllocationGranularity;
        dwFileOffsetLow = alignedOffset;
    }
    int unalignedLen = pFileEntry->Offset - alignedOffset;
    size_t mappedSize = unalignedLen + pFileEntry->CompressedSize + pFileEntry->OriginalSize;
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
    strcpy_s(pVFile->fileName, sizeof(pVFile->fileName), lpString2);
    pCpkFile->lpMapFileBase = 0;
    if (dwOpenMode == ECPKMode_Mapped) {
        void* lpMapped = MapViewOfFile(fileMappingHandle, 4u, 0, dwFileOffsetLow, mappedSize);// 把文件的一部分map过去
        pCpkFile->lpMapFileBase = lpMapped;
        if (!lpMapped) {
            pCpkFile->bOpened = 0;
            return nullptr;
        }
    }
    pCpkFile->vCRC = pFileEntry->vCRC;
    pCpkFile->fileIndex = currIndex;
    pCpkFile->vParentCRC = pFileEntry->vParentCRC;
    pCpkFile->pRecordEntry = pFileEntry;// 记录entry结构指针
    pCpkFile->pSrc = &(((char *)pCpkFile->lpMapFileBase)[unalignedLen]);

    pCpkFile->bFlag = (pFileEntry->Attrib & 0xFFFF0000) != 0x10000;
    DWORD originalSize = pFileEntry->OriginalSize;
    pCpkFile->srcOffset = unalignedLen;
    pCpkFile->originalSize = originalSize;
    pCpkFile->fileOffset = 0;
    if (pCpkFile->bFlag == 1 && originalSize) {
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
            return 0;
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
            return 0;
        }
    } else {
        pCpkFile->pDest = pCpkFile->pSrc;
    }
    if (dwOpenMode != ECPKMode_Mapped)
        SetFilePointer(fileHandle, pCpkFile->pRecordEntry->Offset, 0, 0);
    ++dwVFileOpened;
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
            result = processCompress(
                (unsigned __int8 *)param->src,
                param->srcSize,
                (BYTE*)param->dest,
                &param->destResultSize,
                (int)&encryptTable);
            if (result) {
                param->bResult = 0;
                return -1;
            }
        } else {
            result = processDeCompress(
                (unsigned __int8 *)param->src,
                param->srcSize,
                (BYTE *)param->dest,
                &param->destResultSize);
            if (result) {
                param->bResult = 0;
                return -1;
            }
        }
    }
    param->bResult = 1;
    return result;
}

int CPK::processCompress(unsigned __int8 *src, unsigned int decompressSize, unsigned char *dest, DWORD *bResult, int encryptTable)
{
    unsigned int v5; // ecx
    unsigned char *v6; // edi
    unsigned char *v7; // esi
    unsigned int v8; // ebx
    unsigned char *v9; // esi
    unsigned __int8 *v10; // ebp
    unsigned __int8 v11; // dl
    unsigned int v12; // eax
    unsigned int v13; // ebx
    unsigned __int8 **v14; // edi
    unsigned int v15; // ecx
    unsigned int v16; // eax
    unsigned int v17; // eax
    unsigned int v18; // eax
    char v19; // dl
    unsigned int v20; // edx
    unsigned int v21; // eax
    unsigned __int8 v22; // al
    char v23; // dl
    char v24; // al
    char v25; // dl
    char v26; // al
    char v27; // dl
    unsigned char *i; // ebx
    unsigned int v29; // ebx
    int v30; // eax
    unsigned int v31; // ecx
    unsigned char *v32; // esi
    unsigned char v33; // ebx
    unsigned int v34; // eax
    unsigned char *v35; // esi
    unsigned int v36; // eax
    char v37; // al
    unsigned int v38; // ecx
    unsigned char *v39; // esi
    unsigned int v40; // ecx
    unsigned __int8 *v41; // ebx
    int v42; // esi
    unsigned __int8 *v43; // eax
    char v44; // cl
    unsigned int v45; // edx
    unsigned int v46; // ecx
    unsigned char *v47; // edi
    unsigned char *v48; // esi
    unsigned int v50; // [esp+10h] [ebp-18h]
    int v51; // [esp+10h] [ebp-18h]
    unsigned __int8 *v52; // [esp+14h] [ebp-14h]
    char v53; // [esp+18h] [ebp-10h]
    unsigned __int8 *v54; // [esp+1Ch] [ebp-Ch]
    int srca; // [esp+2Ch] [ebp+4h]
    unsigned __int8 *encryptTablea; // [esp+3Ch] [ebp+14h]

    v5 = decompressSize;
    v6 = dest;
    v7 = dest;
    if (decompressSize <= 0xD) {
        v8 = decompressSize;
        goto LABEL_61;
    }
    v9 = dest;
    v52 = src;
    v10 = src + 4;
    v54 = &src[decompressSize];
    do {
        while (1) {
            v11 = v10[3];
            v12 = (33 * (*v10 ^ 32 * (v10[1] ^ 32 * (v10[2] ^ ((unsigned int)v10[3] << 6)))) >> 5) & 0x3FFF;
            v13 = *(DWORD *)(encryptTable + 4 * v12);
            v14 = (unsigned __int8 **)(encryptTable + 4 * v12);
            if (v13 < (unsigned int)src)
                goto LABEL_58;
            v15 = (unsigned int)&v10[-(int)v13];
            v50 = (unsigned int)&v10[-(int)v13];
            if (v10 == (unsigned __int8 *)v13 || v15 > 0xBFFF)
                goto LABEL_58;
            if (v15 > 0x800 && *(unsigned char *)(v13 + 3) != v11)
                break;
        LABEL_15:
            if (*(unsigned short *)v13 != *(unsigned short *)v10 || *(unsigned char *)(v13 + 2) != v10[2])
                goto LABEL_58;
            *v14 = v10;
            v18 = v10 - v52;
            if (v10 - v52 > 0) {
                if (v18 > 3) {
                    if (v18 > 0x12) {
                        *v9++ = 0;
                        v53 = v18 - 18;
                        if (v18 - 18 > 0xFF) {
                            v20 = (v18 - 19) / 0xFF;
                            memset(v9, 0, v20);
                            v21 = (v18 - 19) / 0xFF;
                            v9 += v20;
                            do {
                                --v21;
                                ++v53;
                            } while (v21);
                            v15 = v50;
                            v18 = v10 - v52;
                        }
                        v19 = v53;
                    } else {
                        v19 = v18 - 3;
                    }
                    *v9++ = v19;
                } else {
                    *(v9 - 2) |= v18;
                }
                do {
                    ++v9;
                    --v18;
                    *(v9 - 1) = *v52++;
                } while (v18);
            }
            v22 = v10[3];
            v10 += 4;
            if (*(unsigned char *)(v13 + 3) == v22) {
                v23 = *v10++;
                if (*(unsigned char *)(v13 + 4) == v23) {
                    v24 = *v10++;
                    if (*(unsigned char *)(v13 + 5) == v24) {
                        v25 = *v10++;
                        if (*(unsigned char *)(v13 + 6) == v25) {
                            v26 = *v10++;
                            if (*(unsigned char *)(v13 + 7) == v26) {
                                v27 = *v10++;
                                if (*(unsigned char *)(v13 + 8) == v27) {
                                    for (i = (unsigned char *)(v13 + 9); v10 < v54; ++v10) {
                                        if (*i != *v10)
                                            break;
                                        ++i;
                                    }
                                    v29 = v10 - v52;
                                    if (v50 > 0x4000) {
                                        v34 = v50 - 0x4000;
                                        v51 = v50 - 0x4000;
                                        if (v29 <= 9) {
                                            v31 = v51;
                                            *v9 = (v29 - 2) | (v34 >> 11) & 8 | 0x10;
                                            v32 = v9 + 1;
                                            goto LABEL_55;
                                        }
                                        v33 = v29 - 9;
                                        *v9 = (v34 >> 11) & 8 | 0x10;
                                    } else {
                                        v30 = v50 - 1;
                                        v51 = v50 - 1;
                                        if (v29 <= 0x21) {
                                            v31 = v30;
                                            *v9 = (v29 - 2) | 0x20;
                                            v32 = v9 + 1;
                                        LABEL_55:
                                            *v32 = 4 * v31;
                                            v39 = v32 + 1;
                                            v40 = v31 >> 6;
                                            goto LABEL_56;
                                        }
                                        v33 = v29 - 33;
                                        *v9 = 32;
                                    }
                                    v35 = v9 + 1;
                                    if (v33 > 0xFF) {
                                        memset(v35, 0, 4 * ((v33 - 1) / 0x3FC) + ((v33 - 1) / 0xFF & 3));
                                        v36 = (v33 - 1) / 0xFF;
                                        v35 += v36;
                                        do {
                                            v33 = v33 + 1;
                                            --v36;
                                        } while (v36);
                                    }
                                    v31 = v51;
                                    *v35 = v33;
                                    v32 = v35 + 1;
                                    goto LABEL_55;
                                }
                            }
                        }
                    }
                }
            }
            v37 = (unsigned char)--v10 - (unsigned char)v52;
            if (v15 > 0x800) {
                if (v15 > 0x4000) {
                    v31 = v15 - 0x4000;
                    *v9 = (v37 - 2) | (v31 >> 11) & 8 | 0x10;
                } else {
                    v31 = v15 - 1;
                    *v9 = (v37 - 2) | 0x20;
                }
                v32 = v9 + 1;
                goto LABEL_55;
            }
            v38 = v15 - 1;
            *v9 = 4 * (v38 & 7) | 32 * (v37 + 7);
            v39 = v9 + 1;
            v40 = v38 >> 3;
        LABEL_56:
            v41 = &src[decompressSize];
            *v39 = v40;
            v9 = v39 + 1;
            v52 = v10;
            if (v10 >= v54 - 13)
                goto LABEL_60;
        }
        v16 = (33 * (*v10 ^ 32 * (v10[1] ^ 32 * (v10[2] ^ ((unsigned int)v10[3] << 6)))) >> 5) & 0x7FF ^ 0x201F;
        v13 = *(DWORD *)(encryptTable + 4 * v16);
        v14 = (unsigned __int8 **)(encryptTable + 4 * v16);
        if (v13 >= (unsigned int)src) {
            v17 = (unsigned int)&v10[-(int)v13];
            v50 = (unsigned int)&v10[-(int)v13];
            if (v10 != (unsigned __int8 *)v13 && v17 <= 0xBFFF && (v17 <= 0x800 || *(unsigned char *)(v13 + 3) == v11)) {
                v15 = (unsigned int)&v10[-(int)v13];
                goto LABEL_15;
            }
        }
    LABEL_58:
        *v14 = v10++;
    } while (v10 < v54 - 13);
    v41 = &src[decompressSize];
LABEL_60:
    v42 = v9 - dest;
    *bResult = v42;
    v8 = v41 - v52;
    v5 = decompressSize;
    v7 = &dest[v42];
    v6 = dest;
LABEL_61:
    if (v8) {
        v43 = &src[v5 - v8];
        encryptTablea = &src[v5 - v8];
        if (v7 == v6 && v8 <= 0xEE) {
            v44 = v8 + 17;
            goto LABEL_73;
        }
        if (v8 > 3) {
            if (v8 > 0x12) {
                v44 = v8 - 18;
                *v7++ = 0;
                srca = v8 - 18;
                if (v8 - 18 > 0xFF) {
                    v45 = (v8 - 19) / 0xFF;
                    v46 = (v8 - 19) / 0x3FC;
                    memset(v7, 0, 4 * v46);
                    v47 = &v7[4 * v46];
                    v7 += v45;
                    memset(v47, 0, v45 & 3);
                    do {
                        v44 = srca + 1;
                        --v45;
                        srca -= 255;
                    } while (v45);
                    v43 = encryptTablea;
                    v6 = dest;
                }
            LABEL_73:
                *v7 = v44;
            } else {
                *v7 = v8 - 3;
            }
            ++v7;
        } else {
            *(v7 - 2) |= v8;
        }
        do {
            *v7++ = *v43++;
            --v8;
        } while (v8);
    }
    *v7 = 17;
    v48 = v7 + 1;
    *v48++ = 0;
    *v48 = 0;
    *bResult = v48 - v6 + 1;
    return 0;
}

int CPK::processDeCompress(unsigned __int8 *src, int decompressSize, unsigned char *dest, DWORD *resultSize)
{
    unsigned __int8 *v4; // esi
    unsigned __int8 *v5; // ebp
    unsigned char *v6; // eax
    unsigned int v7; // ecx
    unsigned int v8; // ecx
    unsigned __int8 *v9; // esi
    unsigned __int8 v10; // dl
    int v11; // edx
    int v12; // edx
    unsigned int v13; // ecx
    int v14; // edx
    unsigned char *v15; // edx
    unsigned char *v16; // eax
    unsigned char *v17; // edi
    unsigned int v18; // ecx
    unsigned char *v19; // eax
    unsigned char *v20; // edi
    unsigned __int8 v21; // dl
    int v22; // edx
    int v23; // edi
    unsigned __int8 v24; // dl
    int v25; // edx
    unsigned __int16 v26; // dx
    unsigned int v27; // edi
    int v28; // edx
    unsigned char *v29; // edi
    unsigned int v30; // ecx
    char v32; // eax

    v4 = src;
    *resultSize = 0;
    v5 = &src[decompressSize];
    v6 = dest;
    if (*src > 0x11u) {
        v7 = *src - 17;
        v4 = src + 1;
        if (v7 < 4)
            goto LABEL_21;
        do {
            *v6++ = *v4++;
            --v7;
        } while (v7);
        goto LABEL_17;
    }
LABEL_5:
    v8 = *v4;
    v9 = v4 + 1;
    if (v8 < 0x10) {
        if (!v8) {
            if (!*v9) {
                do {
                    v10 = v9[1];
                    v8 += 255;
                    ++v9;
                } while (!v10);
            }
            v11 = *v9++;
            v8 += v11 + 15;
        }
        v12 = *(DWORD *)v9;
        v4 = v9 + 4;
        *(DWORD *)v6 = v12;
        v6 += 4;
        v13 = v8 - 1;
        if (v13) {
            if (v13 < 4) {
                do {
                    *v6++ = *v4++;
                    --v13;
                } while (v13);
            } else {
                do {
                    v13 -= 4;
                    *(DWORD *)v6 = *(DWORD *)v4;
                    v6 += 4;
                    v4 += 4;
                } while (v13 >= 4);
                for (; v13; --v13)
                    *v6++ = *v4++;
            }
        }
    LABEL_17:
        v8 = *v4;
        v9 = v4 + 1;
        if (v8 < 0x10) {
            v14 = (int)&v6[-(int)(v8 >> 2) + -4 * *v9];
            v4 = v9 + 1;
            *v6++ = *(unsigned char *)(v14 - 2049);
            v15 = (unsigned char *)(v14 - 2049 + 1);
        LABEL_19:
            *v6 = *v15;
            v16 = v6 + 1;
            *v16 = v15[1];
            v6 = v16 + 1;
            goto LABEL_20;
        }
    }
    while (1) {
        if (v8 >= 0x40) {
            v17 = &v6[-(int)((v8 >> 2) & 7) - 1 + -8 * *v9];
            v4 = v9 + 1;
            v18 = (v8 >> 5) - 1;
        LABEL_25:
            *v6 = *v17;
            v19 = v6 + 1;
            *v19 = v17[1];
            v6 = v19 + 1;
            v20 = v17 + 2;
            do {
                *v6++ = *v20++;
                --v18;
            } while (v18);
            goto LABEL_20;
        }
        if (v8 < 0x20)
            break;
        v18 = v8 & 0x1F;
        if (!v18) {
            if (!*v9) {
                do {
                    v21 = v9[1];
                    v18 += 255;
                    ++v9;
                } while (!v21);
            }
            v22 = *v9++;
            v18 += v22 + 31;
        }
        v17 = &v6[-(int)((unsigned int)*(unsigned __int16 *)v9 >> 2) - 1];
        v4 = v9 + 2;
    LABEL_41:
        if (v18 < 6 || v6 - v17 < 4)
            goto LABEL_25;
        v28 = *(DWORD *)v17;
        v29 = v17 + 4;
        *(DWORD *)v6 = v28;
        v6 += 4;
        v30 = v18 - 2;
        do {
            v30 -= 4;
            *(DWORD *)v6 = *(DWORD *)v29;
            v6 += 4;
            v29 += 4;
        } while (v30 >= 4);
        for (; v30; --v30)
            *v6++ = *v29++;
    LABEL_20:
        v7 = *(v4 - 2) & 3;
        if (!(*(v4 - 2) & 3))
            goto LABEL_5;
        do {
        LABEL_21:
            *v6++ = *v4++;
            --v7;
        } while (v7);
        v8 = *v4;
        v9 = v4 + 1;
    }
    if (v8 < 0x10) {
        v15 = &v6[-(int)(v8 >> 2) - 1 + -4 * *v9];
        v4 = v9 + 1;
        goto LABEL_19;
    }
    v23 = (int)&v6[-2048 * (v8 & 8)];
    v18 = v8 & 7;
    if (!v18) {
        if (!*v9) {
            do {
                v24 = v9[1];
                v18 += 255;
                ++v9;
            } while (!v24);
        }
        v25 = *v9++;
        v18 += v25 + 7;
    }
    v26 = *(unsigned short *)v9;
    v4 = v9 + 2;
    v27 = v23 - ((unsigned int)v26 >> 2);
    if ((unsigned char *)v27 != v6) {
        v17 = (unsigned char *)(v27 - 0x4000);
        goto LABEL_41;
    }
    *resultSize = v6 - dest;
    if (v4 == v5)
        return 0;
    v32 = -(v4 < v5);
    v32 = v32 & 0xFC;
    return v32 - 4;
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
