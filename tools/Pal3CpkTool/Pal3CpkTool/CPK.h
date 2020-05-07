#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <vector>
#include <map>


enum CPKTableFlag {
    CPKTableFlag_None = 0x0,
    CPKTableFlag_IsFile = 0x1,         //�Ƿ��ǺϷ��ļ���
    CPKTableFlag_IsDir = 0x2,          //�Ƿ���Ŀ¼
    CPKTableFlag_Unknown2 = 0x4,
    CPKTableFlag_Unknown3 = 0x8,
    CPKTableFlag_IsDeleted = 0x10,     //�Ƿ���ɾ��

};


struct CPKTable {
    DWORD dwCRC;                    //0x00  ��ǰ�ڵ�CRC
    CPKTableFlag dwFlag;            //0x04  �ļ�������Ϣ
    DWORD dwFatherCRC;              //0x08  ���ڵ�CRC�����ڵ�Ϊ0
    DWORD dwStartPos;               //0x0C  ѹ�����������CPK�е�ƫ������
    DWORD dwPackedSize;             //0x10  ѹ�������ݵĴ�С������Ŀ¼�����ֵΪ0��
    DWORD dwOriginSize;             //0x14  ԭʼ�ļ��Ĵ�С���������ѹʱ����������
    DWORD dwExtraInfoSize;          //0x18  �ļ�����Ϣƫ�� ��Ϣ������ѹ������֮�����Դ�Offset + CompressedSize��ȡInfoRecordSize���ͷ�����ļ���
};

class CPKFile {
public:
    bool bValid;                    //0x110 �Ƿ���Ч
    DWORD dwCRC;                    //0x114 
    DWORD dwFatherCRC;              //0x118 ���ڵ�CRC
    DWORD nTableIndex;              //0x11C �ļ������±�
    LPVOID lpMapAddress;            //0x120 �ļ�ӳ���ַ
    void* lpStartAddress;           //0x124 �ļ�ԭʼ����
    DWORD dwOffset;                 //0x128 ����ƫ����
    bool bCompressed;               //0x12C �Ƿ���ѹ���ļ�
    void* lpMem;                    //0x130 һ���Ž�ѹ������
    DWORD dwFileSize;               //0x134 ԭʼ�ļ���С
    DWORD dwPointer;                //0x138 �ļ�ָ��
    CPKTable* pRecordEntry;         //0x13C �ļ��ṹָ��
};

//0x140
struct gbVFile : CPKFile {
    DWORD OpenMode;                 //0x0
    DWORD EntryAddr;                 //0x4
    DWORD FileSize;                 //0x8
    char fileName[MAX_PATH];        //0xC       �ļ���
    CPKFile cpkFile;
};


struct CpkZipUnzipParam {
    int flag;                       //0x00  һ��Ϊ2����CpkFileEntry::Attrib��HIWORD����
    bool bCompress;                 //0x04  �Ƿ�����ѹ��
    void* src;                      //0x08  �ļ�Դ����ָ��
    void* dest;                     //0x0C  �ļ�Ŀ������ָ��
    DWORD srcSizeUnused;            //0x10  ��ʱδ������ʹ��
    DWORD destSize;                 //0x14  Ŀ�����ݴ�С
    DWORD srcSize;                  //0x18  Դ���ݴ�С
    DWORD destResultSize;           //0x1C  ʵ�ʵõ������ݴ�С
    bool bResult;                   //0x20  �����Ƿ�ɹ�
};

//0x80
struct CPKHeader {
    DWORD dwLable;           //0x0
    DWORD dwVersion;         //0x4   �汾 ����Ϊ1
    DWORD dwTableStart;      //0x08
    DWORD dwDataStart;       //0x0C
    DWORD dwMaxFileNum;      //0x10  ����ļ�����
    DWORD dwFileNum;         //0x14  �ļ�����
    DWORD dwIsFormatted;     //0x18
    DWORD dwSizeOfHeader;    //0x1C
    DWORD dwValidTableNum;   //0x20  CpkFileEntry��������
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
    bool ReadFileEntryName(const CPKTable* pFileEntry, char* lpBuffer, DWORD bufferLen);




private:
    DWORD dwAllocationGranularity;                  //0x0           ����볤�ȣ����ļ�ӳ��ʱ��Ҫ���뵽�ó��ȣ�����ӳ��ʧ��
    ECPKMode m_eMode;                               //0x4           ��ģʽ ��ǰΪECPKMode_Mapped
    CPKHeader cpkHeader;                            //0x8           �ļ�ͷ��Ϣ
    CPKTable entries[32768];                    //0x88          �ļ��ڵ���Ϣ���飬ͨ����ϣ�洢
    gbVFile* m_pgbVFile[0x8];                       //0xE0088       �ļ�����
    bool m_bLoaded;                                 //0xE00A8       �Ƿ��Ѽ���
    HANDLE m_dwCPKHandle;                           //0xE0090       �ļ����
    HANDLE m_dwCPKMappingHandle;                    //0xE0094       �ļ�ӳ����
    char fileName[MAX_PATH];                        //0xE0098       CPK�ļ���
    DWORD m_nOpenedFileNum;                         //0xE009C       ��ǰ�򿪵�gbVFile�ļ�����

private:
    static DWORD *CrcTable[256];
    static void* lzo_wrkmem;
};

