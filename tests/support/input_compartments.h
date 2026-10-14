#pragma once
// Control input-mode state in the harness while keeping real Windows contexts,
// ranges, edit locks and composition notifications. Desktop TIP notifications
// must not turn a fault-injection fixture into an unrelated mode-switch test.
class InputCompartment final : public ITfCompartment {
    long refs_=1; VARIANT value_{};
public:
    ~InputCompartment(){VariantClear(&value_);}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;if(iid!=IID_IUnknown&&iid!=IID_ITfCompartment)return E_NOINTERFACE;
        *out=static_cast<ITfCompartment*>(this);AddRef();return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override{return InterlockedIncrement(&refs_);}
    ULONG STDMETHODCALLTYPE Release() override{auto n=InterlockedDecrement(&refs_);if(!n)delete this;return n;}
    HRESULT STDMETHODCALLTYPE SetValue(TfClientId,const VARIANT* value) override {VariantClear(&value_);return VariantCopy(&value_,value);}
    HRESULT STDMETHODCALLTYPE GetValue(VARIANT* value) override {VariantInit(value);return VariantCopy(value,&value_);}
};
class InputCompartments final : public ITfCompartmentMgr {
    long refs_=1;ComPtr<ITfCompartmentMgr> real_;
    ComPtr<InputCompartment> chinese_,conversion_;
public:
    InputCompartments(ITfThreadMgr* thread){thread->QueryInterface(IID_PPV_ARGS(&real_));chinese_.Attach(new InputCompartment);conversion_.Attach(new InputCompartment);}
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;if(iid!=IID_IUnknown&&iid!=IID_ITfCompartmentMgr)return E_NOINTERFACE;
        *out=static_cast<ITfCompartmentMgr*>(this);AddRef();return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override{return InterlockedIncrement(&refs_);}
    ULONG STDMETHODCALLTYPE Release() override{auto n=InterlockedDecrement(&refs_);if(!n)delete this;return n;}
    HRESULT STDMETHODCALLTYPE GetCompartment(REFGUID id,ITfCompartment** out) override {
        if(id==GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)return chinese_.CopyTo(out);
        if(id==GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)return conversion_.CopyTo(out);
        return real_->GetCompartment(id,out);
    }
    HRESULT STDMETHODCALLTYPE ClearCompartment(TfClientId client,REFGUID id) override {
        if(id==GUID_COMPARTMENT_KEYBOARD_OPENCLOSE||id==GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION){VARIANT empty{};return (id==GUID_COMPARTMENT_KEYBOARD_OPENCLOSE?chinese_:conversion_)->SetValue(client,&empty);}
        return real_->ClearCompartment(client,id);
    }
    HRESULT STDMETHODCALLTYPE EnumCompartments(IEnumGUID** out) override{return real_->EnumCompartments(out);}
};
