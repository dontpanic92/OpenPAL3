#pragma once
/************************************************************************/
/* ��IDA��ȡ��ͷ�ļ�                                                    */
/************************************************************************/

struct CPKTable {
    unsigned int dwCRC;              //���Ҳ²�Ӧ���Ǹ����ļ���Hash����һ����ֵ�����ɸ�Index�ṹ��CPK�ļ��о��ǰ������ֵ�������еġ�
                                    //�����ĺô���ֻҪ�����Ҫ�����ļ���CRC���Ϳ������ö��ֲ����ڶ���ʱ���ڶ�λ���ļ���Index��������ȡ���ݡ�

    unsigned int dwFlag;            //0002,0001�����ļ�, ��������ò����һ����ѹ��һ����δѹ��. ��0011����ɾ�����ļ�, 0003��Ŀ¼. ����Ҳ����0013��ʾ��ɾ����Ŀ¼

    unsigned int dwFatherCRC;        //һ��CRCֵ���������ĸ�Ŀ¼��CRC��CPK�ļ�֧����Ŀ¼�����㶨λ��һ���ļ���index��ͨ�����ָ�뷴�����ϲ������
                                    //�Ϳ���ȡ�����������Ĵ洢·�����ڸ�Ŀ¼�µ��ļ���Index�д�ֵΪ0��

    unsigned int dwStartPos;            //ѹ�����������CPK�е�ƫ������

    unsigned int dwPackedSize;    //ѹ�������ݵĴ�С������Ŀ¼�����ֵΪ0��

    unsigned int dwOriginSize;      //ԭʼ�ļ��Ĵ�С���������ѹʱ����������

    unsigned int dwExtraInfoSize;    /*��ֵĲ���������ÿһ��Index��������ļ���ѹ�����������CPK�д�index.Offset��ʼ�洢��ռ��index.CompressedSize�Ŀռ䣬
                                      ����������һ����СΪInfoRecordSize����ּ�¼����ֻ֪�������¼��һ��ͷ�����ļ�������#0�����������Ķ������������Ȥ�Ŀ����о�һ�¡�
                                      ��Ҫע����ǣ�ֻҪInfoRecordSizeΪ0�������Index����Ŀ¼����CompressedSizeΪ0�����Index�ͺ������壬���账��
                                      ����Ϊ���������������Ϊ�˵��������о�CPK��ʽ�����ļ������кö���������ЧIndex�ˡ�*/
};

struct CPKFile {
    bool bValid;                     //0x110
    DWORD dwCRC;                      //0x114
    DWORD dwFatherCRC;                //0x118
    DWORD nTableIndex;                //0x11C
    LPVOID lpMapAddress;           //0x120
    void* lpStartAddress;                     //0x124
    DWORD dwOffset;                   //0x128
    bool bCompressed;                     //0x12C
    void* lpMem;                    //0x130
    DWORD dwFileSize;             //0x134
    DWORD dwPointer;                   //0x138
    CPKTable* pRecordEntry;     //0x13C
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
struct CPKHeader {
    unsigned int dwLable; //0x0
    DWORD dwVersion;      //0x4  �Ϸ���CPK�ļ��˴�ֵ����Ϊ1
    DWORD unknown[0x2];     //0x08
    DWORD dwMaxFileNum;    //0x10
    DWORD unknown2[0x3];    //0x14
    unsigned int dwValidTableNum;   //0x20
    char unknown3[0x5C];
};


struct CPK {
    unsigned long dwAllocationGranularity;      //0x0
    unsigned long m_eMode;                   //0x4
    CPKHeader cpkHeader;                        //0x8
    CPKTable entries[0x8000];               //0x88
    gbVFile* m_pgbVFile[0x8];                        //0xE0088
    bool m_bLoaded;                              //0xE00A8
    HANDLE m_dwCPKHandle;                          //0xE0090
    HANDLE m_dwCPKMappingHandle;                   //0xE0094
    char fileName[MAX_PATH];                    //0xE0098
    DWORD m_nOpenedFileNum;                        //0xE009C
};