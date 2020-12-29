#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <vector>
#include <map>


enum CPKTableFlag {
    CPKTableFlag_None = 0x0,
    CPKTableFlag_IsFile = 0x1,         //是否是合法文件？
    CPKTableFlag_IsDir = 0x2,          //是否是目录
    CPKTableFlag_Unknown2 = 0x4,
    CPKTableFlag_Unknown3 = 0x8,
    CPKTableFlag_IsDeleted = 0x10,     //是否已删除

};


struct CPKTable {
    DWORD dwCRC;                    //0x00  当前节点CRC
    CPKTableFlag dwFlag;            //0x04  文件属性信息
    DWORD dwFatherCRC;              //0x08  父节点CRC，根节点为0
    DWORD dwStartPos;               //0x0C  压缩后的数据在CPK中的偏移量。
    DWORD dwPackedSize;             //0x10  压缩后数据的大小。对于目录，这个值为0。
    DWORD dwOriginSize;             //0x14  原始文件的大小，方便你解压时开缓冲区。
    DWORD dwExtraInfoSize;          //0x18  文件名信息偏移 信息紧跟着压缩数据之后，所以从Offset + CompressedSize读取InfoRecordSize，最开头就是文件名
};

class CPKFile {
public:
    bool bValid;                    //0x110 是否有效
    DWORD dwCRC;                    //0x114
    DWORD dwFatherCRC;              //0x118 父节点CRC
    DWORD nTableIndex;              //0x11C 文件数组下标
    LPVOID lpMapAddress;            //0x120 文件映射基址
    void* lpStartAddress;           //0x124 文件原始缓冲
    DWORD dwOffset;                 //0x128 对齐偏移量
    bool bCompressed;               //0x12C 是否是压缩文件
    void* lpMem;                    //0x130 一般存放解压缩内容
    DWORD dwFileSize;               //0x134 原始文件大小
    DWORD dwPointer;                //0x138 文件指针
    CPKTable* pRecordEntry;         //0x13C 文件结构指针
};

//0x140
struct gbVFile : CPKFile {
    DWORD OpenMode;                 //0x0
    DWORD EntryAddr;                 //0x4
    DWORD FileSize;                 //0x8
    char fileName[MAX_PATH];        //0xC       文件名
    CPKFile cpkFile;
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
struct CPKHeader {
    DWORD dwLable;           //0x0
    DWORD dwVersion;         //0x4   版本 必须为1
    DWORD dwTableStart;      //0x08
    DWORD dwDataStart;       //0x0C
    DWORD dwMaxFileNum;      //0x10  最大文件数量
    DWORD dwFileNum;         //0x14  文件数量
    DWORD dwIsFormatted;     //0x18
    DWORD dwSizeOfHeader;    //0x1C
    DWORD dwValidTableNum;   //0x20  CpkFileEntry数组数量
    DWORD dwMaxTableNum;     //0x24
    DWORD dwFragmentNum;     //0x28
    DWORD dwPackageSize;     //0x2C
    DWORD dwReserved[20];    //0x30
};

enum ECPKMode {
    CPKM_Null = 0,
    CPKM_Normal = 1,
    CPKM_FileMapping = 2,
    CPKM_Overlapped = 3,
    CPKM_End = 4,
};

enum ECPKSeekFileType {
    ECPKSeekFileType_Set,
    ECPKSeekFileType_Add,
    ECPKSeekFileType_Sub,
};

class CPKDirectoryEntry {

public:
    CPKDirectoryEntry()
        :vCRC(0), vParentCRC(0), lpszName{ 0 }, iAttrib(CPKTableFlag_None)
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
    CPKTableFlag iAttrib;
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

    bool BuildDirectoryTree(CPKDirectoryEntry& entry);
    bool buildParent(CPKTable& currEntry, std::map<DWORD, CPKDirectoryEntry*>& handledEntries);

public:
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
    bool ReadFileEntryName(const CPKTable* pFileEntry, char* lpBuffer, DWORD bufferLen);




private:
    DWORD dwAllocationGranularity;                  //0x0           块对齐长度，做文件映射时需要对齐到该长度，否则映射失败
    ECPKMode m_eMode;                               //0x4           打开模式 当前为ECPKMode_Mapped
    CPKHeader cpkHeader;                            //0x8           文件头信息
    CPKTable entries[32768];                    //0x88          文件节点信息数组，通过哈希存储
    gbVFile* m_pgbVFile[0x8];                       //0xE0088       文件数组
    bool m_bLoaded;                                 //0xE00A8       是否已加载
    HANDLE m_dwCPKHandle;                           //0xE0090       文件句柄
    HANDLE m_dwCPKMappingHandle;                    //0xE0094       文件映射句柄
    char fileName[MAX_PATH];                        //0xE0098       CPK文件名
    DWORD m_nOpenedFileNum;                         //0xE009C       当前打开的gbVFile文件数量

public:
    static DWORD *CrcTable[256];
    static void* lzo_wrkmem;
};

