#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <vector>
#include <map>


enum CpkFileAttrib {
    CpkFileAttrib_None = 0x0,
    CpkFileAttrib_IsFile = 0x1,         //是否是合法文件？
    CpkFileAttrib_IsDir = 0x2,          //是否是目录
    CpkFileAttrib_Unknown2 = 0x4,
    CpkFileAttrib_Unknown3 = 0x8,
    CpkFileAttrib_IsDeleted = 0x10,     //是否已删除

};


struct CpkFileEntry {
    unsigned int vCRC;                  //0x00  当前节点CRC
    CpkFileAttrib Attrib;               //0x04  文件属性信息
    DWORD vParentCRC;                   //0x08  父节点CRC，根节点为0
    unsigned int Offset;                //0x0C  压缩后的数据在CPK中的偏移量。
    unsigned int CompressedSize;        //0x10  压缩后数据的大小。对于目录，这个值为0。
    unsigned int OriginalSize;          //0x14  原始文件的大小，方便你解压时开缓冲区。
    unsigned int InfoRecordSize;        //0x18  文件名信息偏移 信息紧跟着压缩数据之后，所以从Offset + CompressedSize读取InfoRecordSize，最开头就是文件名
};

class CPKFile {
public:
    bool bOpened;                   //0x110     是否打开
    DWORD vCRC;                     //0x114     本节点CRC
    DWORD vParentCRC;               //0x118     父节点CRC
    DWORD fileIndex;                //0x11C     文件数组下标
    LPVOID lpMapFileBase;           //0x120     文件映射基址
    void* pSrc;                     //0x124     文件原始缓冲
    DWORD srcOffset;                //0x128     对齐字节
    bool isCompressed;              //0x12C     是否是压缩文件
    void* pDest;                    //0x130     解压缓冲区
    DWORD originalSize;             //0x134     原始文件大小
    DWORD fileOffset;               //0x138     文件偏移
    CpkFileEntry* pRecordEntry;     //0x13C     文件结构指针
};

//0x140
struct gbVFile {
    DWORD unknown1;                 //0x0
    DWORD unknown2;                 //0x4
    DWORD unknown3;                 //0x8
    char fileName[MAX_PATH];        //0xC       文件名
    CPKFile cpkFile;                //0x110     文件信息结构
};


struct CpkZipUnzipParam {
    int flag;                       //0x00  一般为2，从CpkFileEntry::Attrib的HIWORD复制
    bool bCompress;                 //0x04  是否启用压缩
    void* src;                      //0x08  文件源数据指针
    void* dest;                     //0x0C  文件目标数据指针
    DWORD srcSizeUnused;            //0x10  暂时未发现有使用
    DWORD destSize;                 //0x14  目标数据大小
    DWORD srcSize;                  //0x18  源数据大小
    DWORD destResultSize;           //0x1C  实际得到的数据大小
    bool bResult;                   //0x20  操作是否成功
};

//0x80
struct CpkHeader {
    unsigned int signature; //0x0
    DWORD dwCheckFlag;      //0x4   合法的CPK文件此处值必须为1
    DWORD unknown[0x2];     //0x08
    DWORD entryCapacity;    //0x10  CpkFileEntry数组容量
    DWORD unknown2[0x3];    //0x14
    unsigned int dwCount;   //0x20  CpkFileEntry数组数量
    char unknown3[0x5C];    //0x24
};

enum ECPKMode {
    ECPKMode_None = 0,
    ECPKMode_File = 1,
    ECPKMode_Mapped = 2,
};

enum ECPKSeekFileType {
    ECPKSeekFileType_Set,
    ECPKSeekFileType_Add,
    ECPKSeekFileType_Sub,
};

class CPKDirectoryEntry {

public:
    CPKDirectoryEntry()
        :vCRC(0), vParentCRC(0), lpszName{ 0 }, iAttrib(CpkFileAttrib_None)
    {
    }
    ~CPKDirectoryEntry()
    {
        for (int i = 0; i < childs.size(); i++)
            delete childs[i];
        childs.clear();
    }
    DWORD vCRC;
    DWORD vParentCRC;
    CpkFileAttrib iAttrib;
    CHAR lpszName[MAX_PATH];
    std::vector<CPKDirectoryEntry*> childs;
};

class CPK {
public:
    CPK();
    ~CPK();
public:

    bool Close(CPKFile *pCpkFile);
    bool IsFileExist(char const *lpString2);
    bool IsLoaded(void);
    static bool IsValidCPK(const char *lpFileName);
    bool Load(char const *lpFileName);
    bool Read(void* lpBuffer, DWORD nNumberOfBytesToRead, CPKFile *pCpkFile);
    bool Unload(void);
    char * ReadLine(char *lpBuffer, int ReadSize, CPKFile *pCpkFile);
    CPKFile* Open(const char *lpString2);
    CPKFile* Open(DWORD vCRC, const char* saveFileName);
    char ReadChar(CPKFile * pCpkFile);
    DWORD Compress(void *dest, void *src, unsigned int size);
    DWORD DeCompress(void *dest, void *src, DWORD compressedSize);
    HANDLE GetCPKHandle();
    DWORD GetSize(CPKFile *pCpkFile);
    DWORD LoadFile(void *lpBuffer, const char *lpString2);
    DWORD Seek(CPKFile *pCpkFile, int seekPos, ECPKSeekFileType seekType);
    DWORD Tell(CPKFile *pCpkFile);
    void Rewind(CPKFile *pCpkFile);
    void SetOpenMode(ECPKMode openMode);

    bool buildDirectoryTree(CPKDirectoryEntry& entry);
    bool buildParent(CpkFileEntry& currEntry, std::map<DWORD, CPKDirectoryEntry*>& handledEntries);

private:
    int executeZipUnZip(CpkZipUnzipParam *param);
    gbVFile* OpenTableIndex(int iFileIndex);


    bool GetFileSize(DWORD &CompressedSize, DWORD &OriginalSize, DWORD targetCRC);
    bool IsDir(DWORD dwTargetCRC);
    int GetTableIndex(const char *lpString2);
    int GetTableIndexFromCRC(DWORD dwTargetCRC);
    static DWORD Crc(const char *name);
    static void InitCrcTable(void);
    DWORD GetAllocationGranularity(void);
    void Reset();
    bool ReadFileEntryName(const CpkFileEntry* pFileEntry, char* lpBuffer, DWORD bufferLen);




private:
    DWORD dwAllocationGranularity;                  //0x0           块对齐长度，做文件映射时需要对齐到该长度，否则映射失败
    ECPKMode dwOpenMode;                            //0x4           打开模式 当前为ECPKMode_Mapped
    CpkHeader cpkHeader;                            //0x8           文件头信息
    CpkFileEntry entries[0x8000];                   //0x88          文件节点信息数组，通过哈希存储
    gbVFile* vFiles[0x8];                           //0xE0088       文件数组
    bool isLoaded;                                  //0xE00A8       是否已加载
    HANDLE fileHandle;                              //0xE0090       文件句柄
    HANDLE fileMappingHandle;                       //0xE0094       文件映射句柄
    char fileName[MAX_PATH];                        //0xE0098       CPK文件名
    DWORD dwVFileOpenedCount;                       //0xE009C       当前打开的gbVFile文件数量

private:
    static DWORD *CrcTable[256];
    static void* lzo_wrkmem;
};

