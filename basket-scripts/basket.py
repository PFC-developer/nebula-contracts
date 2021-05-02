from terra_sdk.core.wasm import dict_to_b64


class Oracle:
    @staticmethod
    def set_prices(prices):
        return {"set_prices": {"prices": prices}}

class Asset:
    @staticmethod
    def cw20_asset_info(haddr):
        return {"token": {"contract_addr": haddr}}

    @staticmethod
    def native_asset_info(denom):
        return {"native_token": {"denom": denom}}

    @staticmethod
    def asset(string, amount, native=False):
        if not native:
            return {"info": Asset.cw20_asset_info(string), "amount": amount}
        else:
            return {"info": Asset.native_asset_info(string), "amount": amount}

class CW20:
    @staticmethod
    def transfer(recipient, amount):
        return {"transfer": {"recipient": recipient, "amount": amount}}

    @staticmethod
    def send(contract, amount, msg_data=None):
        msg = None
        if msg_data is not None:
            msg = dict_to_b64(msg_data)
        return {"send": {"contract": contract, "amount": amount, "msg": msg}}


class Basket:
    @staticmethod
    def set_basket_token(basket_token):
        return {"__set_basket_token": {"basket_token": basket_token}}

    @staticmethod
    def mint(asset_amounts):
        return {"mint": {"asset_amounts": asset_amounts}}

    @staticmethod
    def stage_asset():
        return {"stage_asset": {}}

    @staticmethod
    def burn(asset_amounts=None):
        return {"burn": {"asset_amounts": asset_amounts}}

    @staticmethod
    def stage_native_asset(denom, amount):
        return {"stage_native_asset": {"asset": Asset.asset(denom, amount, native=True)}}

    @staticmethod
    def reset_target(new_assets, new_target):
        return {"reset_target": {"assets": new_assets, "target": new_target}}