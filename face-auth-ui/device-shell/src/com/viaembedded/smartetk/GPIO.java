package com.viaembedded.smartetk;

import java.util.ArrayList;

/* loaded from: classes.dex */
public class GPIO extends ServiceIO {
    private static final byte CMD_GPIO_GETDIRECTION = 6;
    private static final byte CMD_GPIO_GETENABLE = 7;
    private static final byte CMD_GPIO_GETINFO = 1;
    private static final byte CMD_GPIO_GETVALUE = 5;
    private static final byte CMD_GPIO_SETDIRECTION = 3;
    private static final byte CMD_GPIO_SETENABLE = 2;
    private static final byte CMD_GPIO_SETVALUE = 4;
    public static final int GM_GPI = 0;
    public static final int GM_GPIO = 255;
    public static final int GM_GPO = 1;
    public static final int GM_PULL_DOWN = 0;
    public static final int GM_PULL_UP = 1;

    @Override // com.viaembedded.smartetk.ServiceIO
    public /* bridge */ /* synthetic */ void closeService() {
        super.closeService();
    }

    @Override // com.viaembedded.smartetk.ServiceIO
    public /* bridge */ /* synthetic */ void resetService() {
        super.resetService();
    }

    public GPIO() {
        super((byte) 1);
    }

    public int queryGPIOList(ArrayList<Integer> arrayList, ArrayList<Integer> arrayList2) {
        return requestToGetByteArray((byte) 1, arrayList, arrayList2);
    }

    public int setEnable(int i, boolean z) {
        return requestToSetByteBoolean((byte) 2, i, z ? 1 : 0);
    }

    public int setDirection(int i, int i2) {
        return requestToSetByteBoolean((byte) 3, i, i2);
    }

    public int setValue(int i, int i2) {
        return requestToSetByteBoolean((byte) 4, i, i2);
    }

    public int getValue(int i, int[] iArr) {
        return requestToGetByte((byte) 5, i, iArr);
    }

    public int getDirection(int i, int[] iArr) {
        return requestToGetByte((byte) 6, i, iArr);
    }

    public int getEnable(int i, boolean[] zArr) {
        return requestToGetBoolean((byte) 7, i, zArr);
    }
}
