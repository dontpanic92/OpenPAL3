#pragma once
/************************************************************************/
/* ��IDA��ȡ��ͷ�ļ�                                                    */
/************************************************************************/

struct CpkFileEntry {
    unsigned int vCRC;              //���Ҳ²�Ӧ���Ǹ����ļ���Hash����һ����ֵ�����ɸ�Index�ṹ��CPK�ļ��о��ǰ������ֵ�������еġ�
                                    //�����ĺô���ֻҪ�����Ҫ�����ļ���CRC���Ϳ������ö��ֲ����ڶ���ʱ���ڶ�λ���ļ���Index��������ȡ���ݡ�

    unsigned int Attrib;            //0002,0001�����ļ�, ��������ò����һ����ѹ��һ����δѹ��. ��0011����ɾ�����ļ�, 0003��Ŀ¼. ����Ҳ����0013��ʾ��ɾ����Ŀ¼

    unsigned int vParentCRC;        //һ��CRCֵ���������ĸ�Ŀ¼��CRC��CPK�ļ�֧����Ŀ¼�����㶨λ��һ���ļ���index��ͨ�����ָ�뷴�����ϲ������
                                    //�Ϳ���ȡ�����������Ĵ洢·�����ڸ�Ŀ¼�µ��ļ���Index�д�ֵΪ0��

    unsigned int Offset;            //ѹ�����������CPK�е�ƫ������

    unsigned int CompressedSize;    //ѹ�������ݵĴ�С������Ŀ¼�����ֵΪ0��

    unsigned int OriginalSize;      //ԭʼ�ļ��Ĵ�С���������ѹʱ����������

    unsigned int InfoRecordSize;    /*��ֵĲ���������ÿһ��Index��������ļ���ѹ�����������CPK�д�index.Offset��ʼ�洢��ռ��index.CompressedSize�Ŀռ䣬
                                      ����������һ����СΪInfoRecordSize����ּ�¼����ֻ֪�������¼��һ��ͷ�����ļ�������#0�����������Ķ������������Ȥ�Ŀ����о�һ�¡�
                                      ��Ҫע����ǣ�ֻҪInfoRecordSizeΪ0�������Index����Ŀ¼����CompressedSizeΪ0�����Index�ͺ������壬���账��
                                      ����Ϊ���������������Ϊ�˵��������о�CPK��ʽ�����ļ������кö���������ЧIndex�ˡ�*/
};

struct CPKFile {
    bool bOpened;                     //0x110
    DWORD vCRC;                      //0x114
    DWORD vParentCRC;                //0x118
    DWORD fileIndex;                //0x11C
    LPVOID lpMapFileBase;           //0x120
    void* pSrc;                     //0x124
    DWORD srcOffset;                   //0x128
    bool isCompressed;                     //0x12C
    void* pDest;                    //0x130
    DWORD originalSize;             //0x134
    DWORD fileOffset;                   //0x138
    CpkFileEntry* pRecordEntry;     //0x13C
};

//0x140
struct gbVFile {
    DWORD unknown1;                 //0x0
    DWORD unknown2;                 //0x4
    DWORD unknown3;                 //0x8
    char fileName[MAX_PATH];        //0xC
    CPKFile cpkFile;
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
    char unknown3[0x5C];
};


struct CPK {
    unsigned long dwAllocationGranularity;      //0x0
    unsigned long dwOpenMode;                   //0x4
    CpkHeader cpkHeader;                        //0x8
    CpkFileEntry entries[0x8000];               //0x88
    gbVFile* vFiles[0x8];                        //0xE0088
    bool isLoaded;                              //0xE00A8
    HANDLE fileHandle;                          //0xE0090
    HANDLE fileMappingHandle;                   //0xE0094
    char fileName[MAX_PATH];                    //0xE0098
    DWORD dwVFileOpened;                        //0xE009C
};