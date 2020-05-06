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
    unsigned int vCRC;           //���Ҳ²�Ӧ���Ǹ����ļ���Hash����һ����ֵ�����ɸ�Index�ṹ��CPK�ļ��о��ǰ������ֵ�������еġ�
                                //�����ĺô���ֻҪ�����Ҫ�����ļ���CRC���Ϳ������ö��ֲ����ڶ���ʱ���ڶ�λ���ļ���Index��������ȡ���ݡ�

    unsigned int Attrib;        //0002,0001�����ļ�, ��������ò����һ����ѹ��һ����δѹ��. ��0011����ɾ�����ļ�, 0003��Ŀ¼. ����Ҳ����0013��ʾ��ɾ����Ŀ¼

    unsigned int vParentCRC;     //һ��CRCֵ���������ĸ�Ŀ¼��CRC��CPK�ļ�֧����Ŀ¼�����㶨λ��һ���ļ���index��ͨ�����ָ�뷴�����ϲ������
                            //�Ϳ���ȡ�����������Ĵ洢·�����ڸ�Ŀ¼�µ��ļ���Index�д�ֵΪ0��

    unsigned int Offset;        //ѹ�����������CPK�е�ƫ������

    unsigned int CompressedSize;//ѹ�������ݵĴ�С������Ŀ¼�����ֵΪ0��

    unsigned int OriginalSize;  //ԭʼ�ļ��Ĵ�С���������ѹʱ����������

    unsigned int InfoRecordSize;/*��ֵĲ���������ÿһ��Index��������ļ���ѹ�����������CPK�д�index.Offset��ʼ�洢��ռ��index.CompressedSize�Ŀռ䣬
                              ����������һ����СΪInfoRecordSize����ּ�¼����ֻ֪�������¼��һ��ͷ�����ļ�������#0�����������Ķ������������Ȥ�Ŀ����о�һ�¡�
                              ��Ҫע����ǣ�ֻҪInfoRecordSizeΪ0�������Index����Ŀ¼����CompressedSizeΪ0�����Index�ͺ������壬���账��
                              ����Ϊ���������������Ϊ�˵��������о�CPK��ʽ�����ļ������кö���������ЧIndex�ˡ�*/
};

struct CPKFile {
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
    DWORD dwCheckFlag;      //0x4  �Ϸ���CPK�ļ��˴�ֵ����Ϊ1
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

