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
    unsigned int vCRC;           //据我猜测应该是根据文件名Hash出的一个数值，若干个Index结构在CPK文件中就是按这个数值升序排列的。
                                //这样的好处是只要计算出要访问文件的CRC，就可以利用二分查找在对数时间内定位该文件的Index，进而读取数据。

    unsigned int Attrib;        //0002,0001都是文件, 区别忘了貌似是一个是压缩一个是未压缩. 而0011是已删除的文件, 0003是目录. 或许也会有0013表示已删除的目录

    unsigned int vParentCRC;     //一个CRC值，等于它的父目录的CRC。CPK文件支持子目录，当你定位好一个文件的index后，通过这个指针反复向上层遍历，
                            //就可以取得它的完整的存储路径。在根目录下的文件的Index中此值为0。

    unsigned int Offset;        //压缩后的数据在CPK中的偏移量。

    unsigned int CompressedSize;//压缩后数据的大小。对于目录，这个值为0。

    unsigned int OriginalSize;  //原始文件的大小，方便你解压时开缓冲区。

    unsigned int InfoRecordSize;/*奇怪的参数。对于每一个Index所代表的文件，压缩后的数据在CPK中从index.Offset起开始存储，占用index.CompressedSize的空间，
                              接下来就是一个大小为InfoRecordSize的奇怪记录，我只知道这个记录的一开头就是文件名，以#0结束，其他的都不清楚，有兴趣的可以研究一下。
                              需要注意的是，只要InfoRecordSize为0，或这个Index不是目录，但CompressedSize为0，这个Index就毫无疑义，不需处理。
                              我因为多次运行升级程序（为了调试它来研究CPK格式），文件中已有好多这样的无效Index了。*/
};

struct CPKFile {
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
    CPKFile cpkFile;                //0x110
};


struct CpkZipUnzipParam {
    int flag;
    bool bCompress;
    void* src;
    void* dest;
    DWORD srcSizeUnused;
    DWORD destSize;
    DWORD srcSize;
    DWORD destResultSize;
    bool bResult;
};

//0x80
struct CpkHeader {
    unsigned int signature; //0x0
    DWORD dwCheckFlag;      //0x4  合法的CPK文件此处值必须为1
    DWORD unknown[0x2];     //0x08
    DWORD entryCapacity;    //0x10
    DWORD unknown2[0x3];    //0x14
    unsigned int dwCount;   //0x20
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
        :vCRC(0), vParentCRC(0), lpszName{ 0 }, iAttrib(0)
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
    DWORD iAttrib;
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
    int processCompress(unsigned __int8 *src, unsigned int decompressSize, unsigned char *dest, DWORD *bResult, int encryptTable);
    int processDeCompress(unsigned __int8 *src, int decompressSize, unsigned char *dest, DWORD *resultSize);

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
    DWORD dwAllocationGranularity;                  //0x0
    ECPKMode dwOpenMode;                            //0x4
    CpkHeader cpkHeader;                            //0x8
    CpkFileEntry entries[0x8000];                   //0x88
    gbVFile* vFiles[0x8];                           //0xE0088
    bool isLoaded;                                  //0xE00A8
    HANDLE fileHandle;                              //0xE0090
    HANDLE fileMappingHandle;                       //0xE0094
    char fileName[MAX_PATH];                        //0xE0098
    DWORD dwVFileOpened;                            //0xE009C

private:
    static DWORD *CrcTable[256];
};

