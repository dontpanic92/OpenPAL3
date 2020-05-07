#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <vector>
#include <map>


enum CpkFileAttrib {
    CpkFileAttrib_None = 0x0,
    CpkFileAttrib_IsFile = 0x1,         //�Ƿ��ǺϷ��ļ���
    CpkFileAttrib_IsDir = 0x2,          //�Ƿ���Ŀ¼
    CpkFileAttrib_Unknown2 = 0x4,
    CpkFileAttrib_Unknown3 = 0x8,
    CpkFileAttrib_IsDeleted = 0x10,     //�Ƿ���ɾ��

};


struct CpkFileEntry {
    unsigned int vCRC;                  //0x00  ��ǰ�ڵ�CRC
    CpkFileAttrib Attrib;               //0x04  �ļ�������Ϣ
    DWORD vParentCRC;                   //0x08  ���ڵ�CRC�����ڵ�Ϊ0
    unsigned int Offset;                //0x0C  ѹ�����������CPK�е�ƫ������
    unsigned int CompressedSize;        //0x10  ѹ�������ݵĴ�С������Ŀ¼�����ֵΪ0��
    unsigned int OriginalSize;          //0x14  ԭʼ�ļ��Ĵ�С���������ѹʱ����������
    unsigned int InfoRecordSize;        //0x18  �ļ�����Ϣƫ�� ��Ϣ������ѹ������֮�����Դ�Offset + CompressedSize��ȡInfoRecordSize���ͷ�����ļ���
};

class CPKFile {
public:
    bool bOpened;                   //0x110     �Ƿ��
    DWORD vCRC;                     //0x114     ���ڵ�CRC
    DWORD vParentCRC;               //0x118     ���ڵ�CRC
    DWORD fileIndex;                //0x11C     �ļ������±�
    LPVOID lpMapFileBase;           //0x120     �ļ�ӳ���ַ
    void* pSrc;                     //0x124     �ļ�ԭʼ����
    DWORD srcOffset;                //0x128     �����ֽ�
    bool isCompressed;              //0x12C     �Ƿ���ѹ���ļ�
    void* pDest;                    //0x130     ��ѹ������
    DWORD originalSize;             //0x134     ԭʼ�ļ���С
    DWORD fileOffset;               //0x138     �ļ�ƫ��
    CpkFileEntry* pRecordEntry;     //0x13C     �ļ��ṹָ��
};

//0x140
struct gbVFile {
    DWORD unknown1;                 //0x0
    DWORD unknown2;                 //0x4
    DWORD unknown3;                 //0x8
    char fileName[MAX_PATH];        //0xC       �ļ���
    CPKFile cpkFile;                //0x110     �ļ���Ϣ�ṹ
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
struct CpkHeader {
    unsigned int signature; //0x0
    DWORD dwCheckFlag;      //0x4   �Ϸ���CPK�ļ��˴�ֵ����Ϊ1
    DWORD unknown[0x2];     //0x08
    DWORD entryCapacity;    //0x10  CpkFileEntry��������
    DWORD unknown2[0x3];    //0x14
    unsigned int dwCount;   //0x20  CpkFileEntry��������
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
    DWORD dwAllocationGranularity;                  //0x0           ����볤�ȣ����ļ�ӳ��ʱ��Ҫ���뵽�ó��ȣ�����ӳ��ʧ��
    ECPKMode dwOpenMode;                            //0x4           ��ģʽ ��ǰΪECPKMode_Mapped
    CpkHeader cpkHeader;                            //0x8           �ļ�ͷ��Ϣ
    CpkFileEntry entries[0x8000];                   //0x88          �ļ��ڵ���Ϣ���飬ͨ����ϣ�洢
    gbVFile* vFiles[0x8];                           //0xE0088       �ļ�����
    bool isLoaded;                                  //0xE00A8       �Ƿ��Ѽ���
    HANDLE fileHandle;                              //0xE0090       �ļ����
    HANDLE fileMappingHandle;                       //0xE0094       �ļ�ӳ����
    char fileName[MAX_PATH];                        //0xE0098       CPK�ļ���
    DWORD dwVFileOpenedCount;                       //0xE009C       ��ǰ�򿪵�gbVFile�ļ�����

private:
    static DWORD *CrcTable[256];
    static void* lzo_wrkmem;
};

