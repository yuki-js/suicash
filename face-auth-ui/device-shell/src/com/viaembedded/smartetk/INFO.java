package com.viaembedded.smartetk;

import java.util.ArrayList;

/* loaded from: classes.dex */
public class INFO extends ServiceIO {
    private static final byte CMD_GET_BOARD_ID = 3;
    private static final byte CMD_GET_BOARD_INFO = 6;
    private static final byte CMD_GET_BOARD_NAME = 7;
    private static final byte CMD_GET_BOARD_SERIALNUMBER = 5;
    private static final byte CMD_GET_BOARD_VERSION = 4;
    private static final byte CMD_QUERY_SUPPORTED_FUNCTIONS = 2;
    private static final byte CMD_QUERY_SUPPORTED_MODULES = 1;
    public static final byte INFO_FUNCTION_GETBID = 1;

    @Override // com.viaembedded.smartetk.ServiceIO
    public /* bridge */ /* synthetic */ void closeService() {
        super.closeService();
    }

    @Override // com.viaembedded.smartetk.ServiceIO
    public /* bridge */ /* synthetic */ void resetService() {
        super.resetService();
    }

    public INFO() {
        super((byte) 0);
    }

    public int querySupportedModules(long[] jArr) {
        return requestToGetLong((byte) 1, jArr);
    }

    public int querySupportedBID(boolean[] zArr) {
        long[] jArr = new long[1];
        int querySuppotedFunctions = querySuppotedFunctions(jArr);
        if ((jArr[0] & 1) != 0) {
            zArr[0] = true;
        } else {
            zArr[0] = false;
        }
        return querySuppotedFunctions;
    }

    private int querySuppotedFunctions(long[] jArr) {
        return requestToGetLong((byte) 2, jArr);
    }

    public int getBoardID(String[] strArr) {
        return requestToGetString((byte) 3, strArr);
    }

    public int getBoardVersion(String[] strArr) {
        return requestToGetString((byte) 4, strArr);
    }

    public int getBoardSerialNumber(String[] strArr) {
        return requestToGetString((byte) 5, strArr);
    }

    public int getBoardInfo(byte[] bArr) {
        ArrayList<Integer> arrayList = new ArrayList<>();
        int requestToGetByteArray = requestToGetByteArray((byte) 6, arrayList);
        if (bArr == null || bArr.length < arrayList.size()) {
            return -4;
        }
        for (int i = 0; i < arrayList.size(); i++) {
            bArr[i] = (byte) (arrayList.get(i).intValue() & 255);
        }
        return requestToGetByteArray;
    }

    public int getSmartETKSDKVersion(int[] iArr) {
        if (iArr == null || iArr.length != 1) {
            return -4;
        }
        iArr[0] = ((ETK_LIB_VERSION_NUMBER[0] & 255) << 24) | ((ETK_LIB_VERSION_NUMBER[1] & 255) << 16) | ((ETK_LIB_VERSION_NUMBER[2] & 255) << 8);
        return 0;
    }

    public int getBoardName(String[] strArr) {
        return requestToGetString((byte) 7, strArr);
    }
}
