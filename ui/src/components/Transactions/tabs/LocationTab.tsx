import { Plus } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api";
import type { Location, LocationType, Transaction } from "../../../types";

const LOCATION_TYPES: { value: LocationType; label: string }[] = [
  { value: "store", label: "Store" },
  { value: "online", label: "Online" },
  { value: "home", label: "Home" },
  { value: "work", label: "Work" },
  { value: "travel", label: "Travel" },
];

interface LocationTabProps {
  transaction: Transaction;
  locations: Location[];
  loading: boolean;
  onLocationsChange: (locations: Location[]) => void;
  onError: (error: string | null) => void;
}

export function LocationTab({
  transaction,
  locations,
  loading,
  onLocationsChange,
  onError,
}: LocationTabProps) {
  const [saving, setSaving] = useState(false);
  const [purchaseLocationId, setPurchaseLocationId] = useState<number | null>(
    transaction.purchase_location_id
  );
  const [vendorLocationId, setVendorLocationId] = useState<number | null>(
    transaction.vendor_location_id
  );
  const [showNewLocation, setShowNewLocation] = useState(false);
  const [newLocation, setNewLocation] = useState({
    name: "",
    address: "",
    city: "",
    state: "",
    location_type: "store" as LocationType,
  });

  const formatLocation = (loc: Location) => {
    const parts = [];
    if (loc.name) parts.push(loc.name);
    if (loc.city) parts.push(loc.city);
    if (loc.state) parts.push(loc.state);
    return parts.join(", ") || `Location #${loc.id}`;
  };

  const handleSaveLocation = async () => {
    try {
      setSaving(true);
      onError(null);
      await api.updateTransactionLocation(transaction.id, {
        purchase_location_id: purchaseLocationId,
        vendor_location_id: vendorLocationId,
      });
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to save location");
    } finally {
      setSaving(false);
    }
  };

  const handleCreateLocation = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newLocation.name && !newLocation.city) {
      onError("Please enter a name or city");
      return;
    }

    try {
      setSaving(true);
      onError(null);
      const location = await api.createLocation({
        name: newLocation.name || undefined,
        address: newLocation.address || undefined,
        city: newLocation.city || undefined,
        state: newLocation.state || undefined,
        location_type: newLocation.location_type,
      });
      onLocationsChange([...locations, location]);
      setShowNewLocation(false);
      setNewLocation({
        name: "",
        address: "",
        city: "",
        state: "",
        location_type: "store",
      });
    } catch (err) {
      onError(err instanceof Error ? err.message : "Failed to create location");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return <p className="text-hone-500 text-center py-4">Loading...</p>;
  }

  return (
    <div className="space-y-4">
      {/* Purchase Location */}
      <div>
        <label className="block text-sm font-medium text-hone-700 mb-1">
          Purchase Location
        </label>
        <p className="text-xs text-hone-400 mb-2">
          Where you made the purchase (delivery address for online orders)
        </p>
        <select
          value={purchaseLocationId || ""}
          onChange={(e) =>
            setPurchaseLocationId(
              e.target.value ? parseInt(e.target.value) : null
            )
          }
          className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
        >
          <option value="">Not set</option>
          {locations.map((loc) => (
            <option key={loc.id} value={loc.id}>
              {formatLocation(loc)}
            </option>
          ))}
        </select>
      </div>

      {/* Vendor Location */}
      <div>
        <label className="block text-sm font-medium text-hone-700 mb-1">
          Vendor Location
        </label>
        <p className="text-xs text-hone-400 mb-2">
          Where the vendor/merchant is based
        </p>
        <select
          value={vendorLocationId || ""}
          onChange={(e) =>
            setVendorLocationId(
              e.target.value ? parseInt(e.target.value) : null
            )
          }
          className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500"
        >
          <option value="">Not set</option>
          {locations.map((loc) => (
            <option key={loc.id} value={loc.id}>
              {formatLocation(loc)}
            </option>
          ))}
        </select>
      </div>

      {/* Save button */}
      <button
        onClick={handleSaveLocation}
        disabled={saving}
        className="btn-primary w-full disabled:opacity-50"
      >
        {saving ? "Saving..." : "Save Location"}
      </button>

      {/* Add new location */}
      <div className="border-t border-hone-100 pt-4">
        {!showNewLocation ? (
          <button
            onClick={() => setShowNewLocation(true)}
            className="btn-secondary w-full flex items-center justify-center gap-2"
          >
            <Plus className="w-4 h-4" />
            Add New Location
          </button>
        ) : (
          <form onSubmit={handleCreateLocation} className="space-y-3">
            <h4 className="text-sm font-medium text-hone-600">
              New Location
            </h4>
            <div>
              <label className="block text-xs text-hone-500 mb-1">
                Name
              </label>
              <input
                type="text"
                value={newLocation.name}
                onChange={(e) =>
                  setNewLocation({ ...newLocation, name: e.target.value })
                }
                placeholder="e.g., Target, Home, Office"
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
              />
            </div>
            <div>
              <label className="block text-xs text-hone-500 mb-1">
                Address
              </label>
              <input
                type="text"
                value={newLocation.address}
                onChange={(e) =>
                  setNewLocation({
                    ...newLocation,
                    address: e.target.value,
                  })
                }
                placeholder="123 Main St"
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs text-hone-500 mb-1">
                  City
                </label>
                <input
                  type="text"
                  value={newLocation.city}
                  onChange={(e) =>
                    setNewLocation({ ...newLocation, city: e.target.value })
                  }
                  className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-hone-500 mb-1">
                  State
                </label>
                <input
                  type="text"
                  value={newLocation.state}
                  onChange={(e) =>
                    setNewLocation({
                      ...newLocation,
                      state: e.target.value,
                    })
                  }
                  className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
                />
              </div>
            </div>
            <div>
              <label className="block text-xs text-hone-500 mb-1">
                Type
              </label>
              <select
                value={newLocation.location_type}
                onChange={(e) =>
                  setNewLocation({
                    ...newLocation,
                    location_type: e.target.value as LocationType,
                  })
                }
                className="w-full px-3 py-2 border border-hone-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-hone-500 text-sm"
              >
                {LOCATION_TYPES.map((type) => (
                  <option key={type.value} value={type.value}>
                    {type.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setShowNewLocation(false)}
                className="btn-secondary flex-1"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={saving}
                className="btn-primary flex-1 disabled:opacity-50"
              >
                {saving ? "Creating..." : "Create"}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
